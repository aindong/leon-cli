# Architecture: protocol and clients

The wire protocol, events, layout, Lua, the app, `leon attach` and the agent skill. Part of [the architecture](../architecture.md).

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
