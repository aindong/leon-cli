# Architecture

leon is a terminal multiplexer: a long-lived server owns sessions, windows (tabs), split
layouts and terminal panes ("blocks"); clients (the macOS app, the `leon` CLI, Lua scripts,
agents) drive it through one self-describing API. `docs/spec.md` is the interface we match.

## Crates (one Cargo workspace)

| Crate | Job |
|---|---|
| `leon-core` | The object model (server state, sessions, windows, layout tree, blocks), stable IDs and target resolution, terminal blocks (PTY via `portable-pty`, emulation via `alacritty_terminal`), the OSC 7501 program-status parser, and the event log with gapless `seq`. No I/O policy, no sockets. |
| `leon-server` | The API: a method registry where every method carries a description and JSON Schemas (`help.all`), served over a Unix socket. Also runnable in-process. |
| `leon-lua` | Lua 5.1 (`mlua`): `init.lua` config (bindings, sequences, modes, actions), the live config `Runtime`, and `leon do` scripts (`leon.on`, `wait`, timers, `exec`). |
| `leon` (bin) | The CLI: client commands, `leon server run/start/stop/status`, autostart, and `leon attach` (the terminal client). With no arguments it opens the app. |
| `leon-hosts` | Other machines: `ssh://` endpoints, the hosts list, and the `Link` that reaches a host's server over ssh. No GPUI, so the CLI and (later) the app share it. |
| `leon-app` | The GPUI client. It attaches to the server over the socket like any client and paints the screens the server pushes, so programs outlive the app. |

## Decisions

- **Wire protocol.** Newline-delimited JSON over a Unix socket. Request `{"id","method","params"}`,
  response `{"id","result"}` or `{"id","error":{"status","code","message"}}`. Events are pushed on
  the same connection as `{"event":{...},"seq":N}` after a subscribe call. The spec leaves framing
  unspecified; NDJSON is the simplest thing that streams and is easy to debug with `nc -U`.
- **Events.** `session.attach {session_id}` (that session, block events too) and
  `session.attach_all {event_scope: session|all}` subscribe the connection and return a snapshot with
  its `seq`. `seq` is per connection and gapless; events are queued while the state lock is held and
  the snapshot is taken under it, so the snapshot sits exactly between two events. Event bodies use
  the spec's CLI JSON shape (`session_event` → `block_event`). A connection more than 8 MiB behind
  is dropped. Each call that changes a session's view bumps its `revision` once and emits one
  `session_view_changed`.
- **Layout.** A window's tiled layout is a tree of `split` nodes (`horizontal` = side by side,
  `ratio` = the before side's share) with block leaves; focus and zoom are per window. CLI ratios
  (`--ratio PCT`) are the new or moved pane's share.
- **Socket path.** `$LEON_RUNTIME_DIR/server.sock`, else `$XDG_RUNTIME_DIR/leon/server.sock`, else
  `~/.local/state/leon/run/server.sock`. Mode 0600.
- **Terminal emulation.** `alacritty_terminal` (battle-tested, used by Zed). leon scans the PTY byte
  stream for OSC 7501 itself before handing bytes to the emulator, since unknown OSCs are dropped.
- **GPUI build.** `gpui` 0.2.2 with the `runtime_shaders` feature, so building does not need Xcode's
  separate Metal Toolchain download.
- **Async.** `tokio` in the server; PTY readers on their own threads feeding the emulator.
- **Config.** `leon-lua` turns `init.lua` into a plain `Config` (bindings with modes and sequences,
  modes, actions, unbinds, diagnostics); a prefix/sequence clash keeps the newest with a warning.
  `leon config check` exits 1 if the load fails or any `leon.*` call was skipped.
- **Lua at runtime.** The server loads `init.lua` at start (`--config`, else `$LEON_CONFIG`, XDG,
  `~/.config`; a missing file is an empty config) and keeps its Lua state on a thread of its own, one
  per good load. Actions (`client.action`, `leon do ACTION`) and function bindings (`client.key`) are
  jobs on that thread; Lua calls server methods in process. `server.config_reload` loads on a fresh
  thread and swaps only on success, so a failed reload keeps the last good keymap and a wedged thread is
  dropped. **Scripts** (`leon do SCRIPT|-e`) run in the `leon do` process on their own connection, so a
  script can never hang the server. Both sides share the `leon.*` code behind a `Host` trait.
- **Lua bounds.** Every state: no `io.popen`/`os.execute`/`os.exit`, no C modules, `debug.traceback`
  only, text chunks only (Lua 5.1 doesn't verify bytecode), no `newproxy` (a `__gc` finalizer runs
  with hooks off). `leon.exec` runs each program in its own process group and signals the group. Server side: a config load gets 5 s and an action 30 s from its request; an instruction hook
  (also on every coroutine) raises past the deadline, `pcall`/`xpcall`/`coroutine.resume` re-raise it,
  and calls and `leon.exec` get only the time left; 256 MiB memory limit. A long C call (a pathological
  `string.find`) isn't interrupted: its caller still gets a timeout, and the next reload replaces the thread.
- **The app attaches.** `leon` (or `leon app`) first does what `leon server start` does (a running
  server, autostarted or not, is told to stay up; else one starts detached, persistent, restoring
  the saved sessions), then attaches over the socket (`leon_app::remote`). Quitting the app (⌘Q,
  the menu, closing the window) only closes its connections: every program keeps running, and the
  next launch shows the same sessions and screens. "Quit and End All Sessions…" asks once, naming
  how many sessions end, then calls `server.stop`; the app quits whenever the server goes away
  (it re-attaches if only its event connection was dropped). The client uses three connections:
  UI calls (keys, layout changes; one that changes the model refetches it before returning), model
  refetches, and events plus screens, read on a thread. The client keeps a copy of the model
  (sessions, windows, layout trees, blocks, program-status records), refetched after each batch of
  events with `session.list`, `session.view`, `session.list_blocks` and `program_status` rather
  than by applying events one by one. The window shows the active tab's whole split tree; keys go
  to the zoomed block, else the focused one. The view redraws when the reader thread wakes it (or
  every ~8 ms for the spinner and the keymap) and only when something drawn changed.
- **Screen transport.** `block.watch {block_ids}` on a connection replaces the set of blocks whose
  screens it gets; the app watches only the panes on screen. Each screen line is
  `{"screen":{block_id, full, lines, cols, rows:[{line, spans, backgrounds}], cursor, app_cursor,
  bracketed_paste}}`: spans are runs of same-styled text at their start column (fg as a 24-bit RGB integer,
  bold/italic/underline/strikeout), backgrounds are runs of cells. The first line for a block, and
  any after a size change, is `full` (every row); later ones carry only the dirty rows, found by
  diffing the frame against the last one sent on that connection (`leon_core::screen::diff`).
  Screen lines have no `seq`: they are state, not events. Per watched block a task waits on the
  terminal's change counter (a `tokio::sync::watch`) and only marks it dirty; the connection loop
  builds the frame under the terminal's lock, between replies and events, so nothing queues per
  connection beyond "dirty": a slow reader gets fewer, newer screens, never a backlog, and is
  never dropped for them. One block's screens are at least 2 ms apart, so a flood costs at most 500
  frame builds a second while a keystroke's echo after a quiet spell goes at once. The watcher
  holds the terminal weakly, so watching a block never keeps its program alive.
  `dev.leon.terminal.screen` returns the same whole screen once. Colours are resolved on the
  server with leon's one theme (shortcut until themes exist: then the client sends its palette, the
  spec's `set_theme`). Other block methods the app uses: `write`, `resize`, `pwd`, and
  `block.event` with `event: {type: "focus"}`, which clears `done`/`error` the way arriving at a
  pane does, and `event: {type: "paste", data}`, which the server wraps by the program's
  bracketed-paste mode at that moment (the app's copy of the screen may be older). `block.watch`
  and `dev.leon.terminal.screen` are leon's own: the spec's screen stream is `data.connect` over a
  WebSocket, which leon doesn't serve. The app attaches only to a socket in a private dir.
- **Scrollback in the app.** Every terminal numbers its lines: `scrolled` counts the lines ever
  pushed into its scrollback, so the screen's top row is line `scrolled`, the oldest kept is
  `scrolled - history`, and appended output never renumbers a line. The count is taken per 4 KiB
  slice of parsed output as how far the newest history row moved: alacritty rotates rows rather
  than copying them, so a kept row's cells stay at one address, and scrolls that never reach the
  scrollback (a delete-line, a top margin) move no history row. A row pushed out of a full
  scrollback is reused, so alacritty's display offset, pinned at 1 for the slice, says when a slice
  may have pushed the whole scrollback's worth (it caps there); it is set back to 0 whenever the
  primary screen shows, so frames never see it. A slice whose count can't be known (that
  overflow, a history clear, an alt-screen switch) moves the count past anything a client keeps,
  so no old number lands on a new line; a resize keeps `scrolled` at least `history`. Screen updates carry `scrolled`, `history`,
  `alt_screen`, `alternate_scroll` and `mouse`. History goes over in ranges, never streamed:
  `dev.leon.terminal.lines {from, count ≤ 1000}` returns rows by absolute number in the screen's row
  shape. The app keeps per pane the top line while scrolled back (None follows the output), a
  trackpad remainder (pixel deltas add up across events, momentum ones too), and a cache of history
  rows fetched 200 around what it shows, on the UI connection (a reply takes well under a
  millisecond); a new size, an alt-screen switch or a shrinking history drops the cache, the
  selection and the scroll. "↓ N new lines" counts from `scrolled` when the view left the bottom.
- **Selection, find and links** run on the server, which has the whole grid:
  `dev.leon.terminal.selection {kind: simple|word|line|block, anchor, head, text}` is alacritty's
  selection (word characters, lines across wraps, rectangles; half a wide character takes the
  whole one) and its selection-to-text (no newline inside a wrapped line), in absolute lines, so a
  selection stays on its text as output arrives. The app keeps kind, anchor and head, asks for the
  range on each drag step and for the text on ⌘C (or at once with `copy_on_select`, a `leon.set`
  setting carried in `client.keymap`'s `settings`). `dev.leon.terminal.find {query, regex,
  case_sensitive, limit}` is alacritty's regex search over scrollback and screen (a plain query is
  escaped; case is forced either way, overriding alacritty's smart case), matches capped at 10000;
  the app runs it on each keystroke and again at most every 250 ms while output arrives.
  `dev.leon.terminal.link_at {line, col}` is the cell's OSC 8 hyperlink, else a URL regex over its
  logical line (across wraps), trailing punctuation and an unmatched `)` trimmed; a URI over 2 KiB
  or with a control, bidi or invisible character is refused. ⌘-click opens `http(s)` at once and
  anything else only after a prompt that shows it.
- **Mouse reporting.** `block.event` takes the spec's `mouse` (press, release, move; button;
  cell; shift/alt/ctrl) and `wheel` (notches) events, encoded on the server by the program's mode
  at that moment (`leon_core::mouse`), as paste is: X10 (`?9`, which alacritty doesn't parse, so a
  bounded scanner beside the OSC 7501 one follows it), 1000, 1002, 1003; SGR (1006), UTF-8 (1005)
  or the legacy bytes (coordinates past 223 dropped); the wheel as buttons 64/65, or on the
  alternate screen with no mouse mode and `?1007` as arrow keys. The app sends presses, releases
  and moves (all moves only for 1003) when the program asked and shift isn't held and the pane
  isn't scrolled back; otherwise the mouse selects.
- **The terminal client.** `leon attach` (`crates/leon/src/attach/`) is the app's client code drawn
  with ratatui 0.30 (crossterm 0.29 through `ratatui::crossterm`) instead of GPUI: it uses
  `leon_app::remote::Remote` (the same connections, model refetches and `block.watch` screen
  stream), the app's keymap, pane layout (in cells, one-cell dividers) and palette filter, without
  code of its own on the server. It watches only the panes on screen (the picker: the previewed
  block). Drawing is coalesced: one thread reads host input, the model's change hook wakes the loop
  (one wake queued at most), the loop handles everything queued and then draws once, and redraws
  for screen changes alone are at least 16 ms apart. Ratatui diffs each frame against the last, so
  a keystroke's echo costs the host a few cells; over a slow link a write blocks, the model keeps
  only the newest screens meanwhile, and the next frame shows those. Keys are decoded by crossterm
  and re-encoded with `leon_core::keys::encode` using the focused pane's own cursor-key mode, so the
  host's mode doesn't matter; pastes go through `block.event` paste. The **prefix** is
  `--prefix`, else the `attach` mode's `prefix` (`leon.mode`, carried in `client.keymap`'s modes),
  else ctrl+b; after it an `attach/KEY` binding, then a config sequence `prefix>KEY`, then the
  built-in table decides. Other config bindings resolve through the app's keymap; anything unbound
  goes to the pane. **Sizes:** the server has no `own_block` yet, so the rule is the client's own
  size: the TUI resizes a pane only when its own layout of that pane changes (attach, host resize,
  split, zoom, the pane coming back on screen), never because another client resized it, and clips or pads a screen of another
  size. The app re-fits whenever a screen's size differs from its pane, so while both show the
  same tab the app's size wins. **The host terminal** is put back (raw mode off, alt screen left,
  cursor shown, bracketed paste off) on detach, errors, a panic (hook), and SIGHUP, SIGTERM,
  SIGINT, SIGQUIT: those are blocked in every thread (after any server autostart, so the server
  doesn't inherit the mask) and taken by a `sigwait` thread; the exit status is 128 + the signal.
  For SIGKILL a watchdog, forked before any thread starts, sits in its own process group with the
  saved termios and a close-on-exec pipe: `e` means the client entered the alt screen, `x` that it
  restored; EOF without `x` means it died, and the watchdog restores (the escapes only after `e`).
  While attached, stderr points at /dev/null, since the app's client code reports some failures
  with `eprintln!` (which would scribble over the screen); restoring puts it back. No tty (`ssh host leon attach` without `-t`) exits 1 and
  writes nothing to stdout.
- **App keys go through the keymap.** The server loads `init.lua` (`leon server run`).
  The app has no GPUI key table (only ⌘Q): every keystroke becomes an unshifted chord
  (`keymap::chord`) and is resolved against its built-in defaults (`keymap::DEFAULTS`) with the
  config's `client.keymap` on top, fetched off the UI thread about once a second. The app runs its
  own actions (`client.*`, `pane.*`, `session.*`); a config action or Lua function goes to the
  server (`client.action` / `client.key`) without waiting. A pending sequence shows a hint and has
  no timeout; unbound keys go to the pane (`dev.leon.terminal.write`). Program status (OSC 7501)
  comes with the model copy; arriving at a pane clears its `done`/`error` (reason `seen`).
- **App layout and look.** Tabs are drawn in one place (`leon_app::chrome::places`): as rows under
  their session in the sidebar (the default), or with `client.tab_style.toggle` in the titlebar row
  beside the session's name, the sidebar then listing sessions only. Panes are inset tiles
  (`panes::tiles`) with a header each; the channel between two tiles is the divider a drag moves.
  The tab style, pane headers, sidebar width and window frame are the app's own settings, saved to
  `$LEON_STATE_DIR/app.json` (0600, via a temp file and a rename, only into a private dir; a missing
  or bad file gives the defaults; the frame is saved on quit and reused only while half of it is on a
  screen, else the window opens at 75% of the primary screen). Icons are one embedded SVG set
  (`leon_app::icons`), served to GPUI as its assets. Every colour comes from `leon_core::theme`, the brand palette
  (brand/README.md): Coal panes on an Iron window, Amber focus, the Maroon tile for the selected
  tab, Copper/Sage/Wheat/Rose for working/done/blocked/error. The server keeps each terminal's last
  OSC 0/2 title, and `session.list_blocks` reports it with the block's cwd, so tabs and pane headers
  name what the program set.
- **Agent skill.** `leon skill show` prints a guide compiled into the binary. `skill setup` installs a
  SKILL.md stub (marked `<!-- leon-skill: … -->`) that tells the agent to run `leon skill show`, so
  upgrades reach agents without a reinstall, plus a matcher-less SessionStart hook running
  `leon skill hook --agent claude|codex` (prints `LEON_SESSION`/`LEON_BLOCK`, and records the agent's
  resume command, below; outside leon nothing and no server call). Claude
  Code: `$CLAUDE_CONFIG_DIR` (`~/.claude`) `skills/leon` + `settings.json`; Codex: `~/.agents/skills/leon`
  + `$CODEX_HOME` (`~/.codex`) `hooks.json`. Settings edits keep key order, back up once to
  `<file>.leon-backup`, write atomically through symlinks, and refuse files that aren't the expected
  JSON shape. Not yet: OpenCode/Pi, `--project`, `--session-naming`.
- **Persistence.** The server saves its sessions to `$LEON_STATE_DIR/sessions.json` (default
  `~/.local/state/leon`) on every exit through the socket loop (`server stop`, SIGTERM, SIGINT,
  SIGHUP, idle exit) and restores them after binding (`--restore`, default true; `server stop --discard`
  skips the save and deletes the file). Saved: sessions, windows, layout trees (split IDs and
  ratios), labels, focus, zoom, the active tab, revisions, and per block its label, keep-open, cwd,
  command, scrollback and resume command. IDs come back unchanged. Programs don't survive: a
  restored block runs a fresh login shell in its cwd (the shell process's cwd, else the last OSC 7
  path, else where it started); the saved command is never re-run, since repeating `make deploy` on
  a restart would be a surprise and would make the file a place that runs code. The dir must be private (0700, ours) and not
  flocked by another server, or nothing is saved (`server status` shows `state_file: null`); the file is 0600, written via a temp file and
  a rename. A file that is corrupt, from a newer version, or inconsistent is moved to
  `sessions.json.bad-<secs>-<n>` with a warning in the log, never replaced.
  The app takes no part: the server it starts restores at start, before it accepts the app's
  connection, so the app's "open a session when there is none" sees the restored ones. Tests and
  benches use `embedded::start_at`, an in-process server that keeps nothing.
- **Scrollback on restore.** Each block's scrollback and screen (`capture`'s text; for an alt-screen
  app, its screen) is saved as text, capped at 2 MiB per block and 8 MiB over all blocks (each gets
  `min(2 MiB, 8 MiB / blocks)`), newest whole lines kept. Every control character but `\n` (C0, DEL,
  C1) is removed when it is saved and again when it is restored, since the file is outside the
  server's hands in between; so a snapshot can't send an escape, a query or a carriage return. The
  new terminal's emulator is fed the text, then a dim `— restored from <time> —` line, before the
  shell's first output: the text is shown, never sent to the shell. Whatever a terminal showed (a
  printed token too) is on disk in the private 0600 file until the next save replaces it;
  `server stop --discard` forgets it.
- **Resume commands.** A program registers the argv that resumes it with
  `dev.leon.terminal.set_resume {command}` (1 to 64 args, at most 8 KiB of printable ASCII, no
  backslash; null clears it). Under "ask" the user agrees to what they see typed, so it must be what
  runs: no control key, no bidi or invisible character, no byte readline could read as Meta, no
  backslash (fish reads `\'` inside single quotes). The skill hook records `claude --resume
  <session_id>` or `codex resume <session_id>` from the agent's SessionStart JSON (an id of
  `[A-Za-z0-9._-]`, no leading `-`), and only when the hook, its parent or its grandparent is its
  terminal's foreground process group: the interactive agent, not a nested `claude -p` that an
  agent's tool started without a terminal. Restored blocks follow the config's
  `leon.set("resume", …)`, read once the start-up config has loaded and before restoring (the
  restore waits at most 1 s for it, since nothing is served until the restore is done; past that
  nothing is offered, as with `"never"`): `"ask"` (default) types Ctrl-U and the shell-quoted command
  at the new prompt and shows a dim hint, so Enter runs it; `"always"` adds the Enter; `"never"`
  types nothing. Typing waits (up to 60 s, then gives up) until the shell is the PTY's foreground
  process group with canonical mode off, so neither an rc file's picker nor typeahead gets it; zsh
  and fish must also have turned bracketed paste on, which their line editors do and a key an rc
  file reads (oh-my-zsh's update question, compinit's `read -q`) doesn't. bash and sh are taken at
  canonical mode alone. The command stays registered until replaced or cleared; with `"always"` the
  private state file is a place that runs the commands programs registered, which is why `"ask"` is
  the default.
- **Terminal reads.** A thread per terminal only reads the PTY into a bounded queue (64 reads of
  up to 16 KiB); the parser thread feeds what has queued, up to 64 KiB, to the OSC 7501 tracker and
  the emulator in one go. The macOS tty buffers ~1 KiB, so reading and parsing on one thread
  stalled the program while it parsed (docs/performance.md).
- **Packaging.** `scripts/bundle.sh` wraps the one `leon` binary in `Leon.app` (bundle id
  `dev.leon.app`, version from `leon --version`, the brand icon `brand/icon/Leon.icns`, ad-hoc
  signed); opened from Finder it runs with no arguments, which opens the app. `scripts/install.sh`
  copies the app to `~/Applications` and links `~/.local/bin/leon` to the binary inside it. Both
  replace only what they made: an app with our bundle id, or a link whose text is our binary's path.
- **`LEON_BIN_DIR`.** Every terminal gets the server binary's directory as `LEON_BIN_DIR` and at
  the front of `PATH` (empty entries dropped; no PATH at all falls back to the system dirs), so
  `leon` resolves inside the app. A login zsh's `path_helper` moves system dirs ahead of it, so an
  older `leon` in `/usr/local/bin` can win there; the dir is still on PATH.
- **Remote hosts.** `-H HOST` (a label from `leon hosts`, or `ssh://[USER@]HOST[:PORT]`) opens a
  `Link` (`leon-hosts`): a Unix socket in a fresh private dir, `link-XXXXXX/s` under the runtime
  dir (0700, socket 0600), served by threads in the client process. Each connection to it spawns
  `$LEON_SSH` (default `ssh`) as argv, never through a local shell:
  `-T -a -x -o ClearAllForwardings=yes -o ServerAliveInterval=15 -o ServerAliveCountMax=3 [-p PORT]
  -- [USER@]HOST CMD`, and copies bytes both ways, only for a peer of our uid (as the server checks). So every client that takes a socket path works unchanged over ssh: the CLI,
  `leon do` scripts, and `leon_app::remote::Remote` (`leon attach`, and the app). leon adds
  only `-T`, no forwarding (no agent on the host, no port a config's `LocalForward` would bind) and
  the keepalives (a dead Wi-Fi link is noticed in about 45 s); everything else comes from
  `~/.ssh/config`, and passwords stay between ssh and the terminal. Once `leon attach` owns the
  terminal it calls `Link::never_prompt`: later ssh runs get `-o BatchMode=yes`, so a reconnect
  fails and retries instead of reading a password through the TUI's raw keys. CMD is one fixed string
  with nothing from the endpoint in it: `sh -c '…'` finds `leon` on PATH, else
  `$HOME/.local/bin/leon`, else exits 127, which the link reports as "install leon on HOST".
  There `leon server proxy` does what `leon server start` does (a persistent server, restoring
  saved sessions) and bridges stdin and stdout to `server.sock`; the server's peer-uid check holds
  because ssh runs the proxy as the host's user. Only a link's connections until one reaches the
  server may autostart; later ones pass `--autostart=false`, so a server stopped on purpose stays
  stopped. Nothing listens on TCP. An endpoint is parsed into words of `[A-Za-z0-9._-]` (or an
  IPv6 address in brackets) that can't start with `-`, a port 1–65535, and no path: anything that
  could reach ssh as an option or a shell as code is a bad command line (exit 2). The hosts list is
  `$LEON_STATE_DIR/hosts.json` (`{"version":1,"hosts":[{label, endpoints, icon}]}`, spec §2),
  written 0600 via a temp file and a rename, read only from a private dir and a regular file of
  ours, and each endpoint is parsed again when used. With `-H`, `LEON_SESSION`/`LEON_BLOCK` are
  ignored. A link flocks `lock` in its dir while it lives; dropping it ends its accept loop (a
  connection of its own wakes it) and so closes the socket, since the app opens one per connection
  attempt for as long as it runs; a killed client leaves its dir, and the
  next `Link::open` removes `link-*` dirs whose lock it can take (never by connecting: a live link
  answers a connection by running ssh). One ssh session per connection is the price of the simple design
  (`leon attach` opens three); `ControlMaster auto` shares one login.
- **Resume and reconnect.** `session.attach`/`session.attach_all` with `resumable: true` also
  return a `resume` token. Such a stream keeps the last 1 MiB of lines it wrote (taken off the
  queue before the write, so a line lost in a dying pipe is kept). When its connection ends it is
  parked for 60 s, still queueing (the 8 MiB drop still applies). `session.resume {resume, seq}`
  on a connection that hasn't attached moves the queue there under the state lock and puts the
  kept lines after `seq` in front: every event after the last one the client saw comes once, with
  the same seq counter. A resume while the old connection is still open (a half-dead link) takes
  the stream over and the old connection ends. The token lasts for every later drop. An unknown
  or expired token, a `seq` ahead of the stream or older than the kept lines fails with
  `resume_failed` (410). `leon events -H` attaches resumable and, when the link drops, reconnects
  with backoff (100 ms doubling to 5 s, for up to 60 s) and resumes; refused (the server
  restarted), it attaches again and prints a new snapshot line. `leon attach -H` and the app use
  `Remote::connect_retrying` (`leon_app::remote`, the link parts in `remote_link.rs`): the event
  stream is attached resumable, and when a connection drops the reader reopens the fetch, call and
  event connections with the same backoff, then `session.resume`s after the last seq it saw (every
  event once, in order; only a refused resume attaches afresh), refetches the model and re-watches
  the screens (a watch set is per connection). A half-dead link (lost Wi-Fi, a stopped proxy)
  closes nothing, so a watchdog pings an event connection that has been quiet for 5 s and shuts it
  down when 5 s more bring no line: about 15 s to notice, not ssh's 45. Over a link the client's
  calls that nobody waits for (keys, resizes, the focus event, pastes) and layout changes (`act_later`:
  the call, the refetch, then a callback) run in order on one queue thread over the call
  connection, so a slow or stalled link never holds the thread that asked; while the link is down
  they fail at once. Keys typed while the link is down are lost; while attached, stderr is
  /dev/null, so ssh's errors during a reconnect aren't shown.
- **The app's hosts.** `leon_app::hosts` holds the machines the window shows: This Mac (the
  socket the app started with), then each host from the hosts list, read at start and after Add
  Host. Each host is connected on a thread of its own: a `Link` (with `never_prompt`, so ssh runs
  with `BatchMode=yes`: the app has no terminal to type a password into) and `connect_retrying`
  for 60 s, the server's resume hold. A link keeps the last line ssh wrote on stderr
  (`Link::last_error`), which is the reason shown: a host is *connecting* until its first attempt
  ends, *connected*, *reconnecting* while its remote retries (the try and the wait), *unreachable*
  with ssh's words, or *needs login* when BatchMode refused a password, passphrase or host key, with
  the advice to run `leon hosts check LABEL` in a terminal (which uses the same ssh, so once it
  works, an agent or a ControlMaster carries the app's next try). A host that gave up is tried again
  after 2 s, doubling to 60 s. The app never opens a pane that runs ssh for a login: that would be a
  second connection that teaches nothing about the app's own. The sidebar groups sessions under
  their machine's label (a dot for the state, a note under it while not connected; collapsed hosts
  saved in app.json as `collapsed_hosts`), only once a host exists, so with none it looks as it
  did. The window's `app` is the selected session's machine's remote, so New Session, New Tab,
  splits, keys and the palette act on the focused pane's host; leaving a host unwatches its panes.
  While a host reconnects its panes keep their last screens under a stale overlay. Lua bindings
  and config actions run on This Mac's server, the keymap's owner, with a host's panes they get no
  ids (shortcut). The palette lists New Session on each connected machine (`session.new {host}`)
  and Add Host… (`client.host.add`): a label, then an endpoint, each checked as `leon hosts add`
  checks them (`leon_hosts::hosts::add`), and saved through the hosts API.
- **Docs are tested.** `docs/cli.md` is checked against every `--help`; the `sh` blocks in the docs
  run against the real binary (a bad command line fails); `docs/lua.md`'s examples go through
  `leon config check` and `leon do -e`.
- **Naming.** Method, action, event and flag names follow `docs/spec.md`, so scripts and agents written
  for that interface carry over. Env vars are `LEON_*`.

## Verification

`cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` (the loop's
`the verify script` runs all three). Terminal behaviour is tested end to end: spawn a real shell in a PTY,
send input, wait for the screen, assert on `capture`. The GUI is checked by window-only screenshots.
