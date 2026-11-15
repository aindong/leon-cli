# Architecture: protocol and clients

The wire protocol, events, layout, Lua, the app (scrollback, selection, find, links, mouse), `leon
attach` and the agent skill. Part of [the architecture](../architecture.md).

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
  Screen lines have no `seq`: they are state, not events. A client reads at most 16 MiB per line
  (a server may be a box we don't control; the CLI's cap is 64 MiB), so an update whose line would
  pass 4 MiB (only a huge screen of distinctly styled cells gets there) goes as several: the first
  carries `full`, the rest are partial updates of the same size (`screens::encode`), and
  `dev.leon.terminal.lines` replies keep the first rows that fit in 4 MiB (the app asks again from
  the first row it lacks). A line past the cap ends the connection for good rather than re-attaching
  to the same line. Event and screen lines are parsed into their types, not a JSON tree, so memory
  stays near a byte per byte read (replies still become a `Value`: a shortcut). Applying an update
  (`screen::apply`) is linear in the update and the frame: one row per line (the last wins), and a
  row keeps at most `cols` spans and backgrounds inside the line, each span at most a cell's text
  per column. Per watched block a task waits on the
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
  JSON shape. Without `--agent`, an agent whose config dir exists or whose program is on PATH. Not yet: OpenCode/Pi, `--project`, `--session-naming`.

- **Floating panes and drawers (M20a).** Terminal ownership stays in `Session.blocks`;
  `Session.overlays` maps window IDs to floating and docked placements separately from
  the tiled tree. This keeps the shared `Window` type compatible with the M20b app work.
  Every window in `session.view` has `floating` (back to front; last is topmost) and
  `docked` arrays. Floats have `block_id`, `bounds` in window fractions, and `hidden`;
  docks have `block_id`, `side` (left/right/bottom), `size` and `hidden`. Each block is
  placed exactly once; a window can have no tiled tree when all its blocks float.
  Hiding never removes a block or stops its PTY. Showing/focusing raises a float;
  hidden panes cannot retain focus or zoom. Dock collisions fail atomically.
  Mutations emit the existing `session_view_changed` revision event; clients refetch
  the full view. `block_ids` includes tiled leaves then overlays, including hidden ones.
  Attach reads them from the shared client's model (`remote::Session::overlays`) over the
  existing screen/event transport. It reserves dock space (in list order, clamped to remaining
  space), then paints bordered floats above tiles in z-order. Bounds are relative to
  the whole window; borders reduce terminal content by one cell on each edge. Zero-size
  content is clipped. The app's drawing, drags and slides (M20b) are in
  [Floats and drawers in the app](floats.md); no new transport is needed.

  Focus next/previous walks tiles, visible docks, then visible floats in block creation
  order (independent of z-order), honoring `wrap`.
  Directional focus from an overlay returns to the first tiled pane (with only overlays,
  it cycles). Attach retains session context with no visible focus, so shortcuts can
  show hidden drawers or open tabs. The picker includes hidden and visible overlay blocks.

  A focused overlay takes input above a zoomed tile; tile-only zoom keeps its existing
  display/input behavior.
