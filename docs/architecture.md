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
| `leon` (bin) | The CLI: client commands, `leon server run/start/stop/status`, autostart. With no arguments it opens the app. |
| `leon-app` | The GPUI client. It embeds the server in-process (no socket hop for rendering) and still listens on the socket, so the CLI and agents drive the same state. |

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
- **The app owns its server.** `leon` (or `leon app`) serves the socket from the app process and
  renders straight from the in-process grids. If a server with sessions already owns the socket,
  the app exits 1 and says to run `leon server stop`; an idle one (no sessions) is stopped with
  `server.stop {if_idle: true}` and replaced. Closing the window ends every program; `leon server
  stop` from the CLI quits the app. The window shows the active tab's whole split tree; keys go to
  the zoomed block, else the focused one. The view polls each visible pane's change counter and
  each session's revision (~8 ms) and redraws only when something changed.
- **Agent skill.** `leon skill show` prints a guide compiled into the binary. `skill setup` installs a
  SKILL.md stub (marked `<!-- leon-skill: … -->`) that tells the agent to run `leon skill show`, so
  upgrades reach agents without a reinstall, plus a matcher-less SessionStart hook running
  `leon skill hook` (prints `LEON_SESSION`/`LEON_BLOCK`, nothing outside leon, no server call). Claude
  Code: `$CLAUDE_CONFIG_DIR` (`~/.claude`) `skills/leon` + `settings.json`; Codex: `~/.agents/skills/leon`
  + `$CODEX_HOME` (`~/.codex`) `hooks.json`. Settings edits keep key order, back up once to
  `<file>.leon-backup`, write atomically through symlinks, and refuse files that aren't the expected
  JSON shape. Not yet: OpenCode/Pi, `--project`, `--session-naming`.
- **Persistence.** The server saves its sessions to `$LEON_STATE_DIR/sessions.json` (default
  `~/.local/state/leon`) on every exit through the socket loop (`server stop`, SIGTERM, SIGINT,
  SIGHUP, idle exit) and restores them after binding (`--restore`, default true; `server stop --discard`
  skips the save and deletes the file). Saved: sessions, windows, layout trees (split IDs and
  ratios), labels, focus, zoom, the active tab, revisions, and per block its label, keep-open, cwd
  and command. IDs come back unchanged. Programs don't survive: a restored block runs a fresh login
  shell in its cwd (the shell process's cwd, else the last OSC 7 path, else where it started); the
  saved command is never re-run, since repeating `make deploy` on a restart would be a surprise and
  would make the file a place that runs code. The dir must be private (0700, ours) and not
  flocked by another server, or nothing is saved (`server status` shows `state_file: null`); the file is 0600, written via a temp file and
  a rename. A file that is corrupt, from a newer version, or inconsistent is moved to
  `sessions.json.bad-<secs>-<n>` with a warning in the log, never replaced.
  The app's embedded server does the same (`embedded::start`): quitting the app (window close,
  the menu, logout) or `leon server stop` ends the socket loop, which saves before programs end,
  and the next launch restores. Restoring is `serve::Listener::persist`, called before `serve_on`
  and before the app looks at the state, so its "open a session when there is none" sees the
  restored ones. Tests use `embedded::start_at`, which keeps nothing.
- **Terminal reads.** A thread per terminal only reads the PTY into a bounded queue (64 reads of
  up to 16 KiB); the parser thread feeds what has queued, up to 64 KiB, to the OSC 7501 tracker and
  the emulator in one go. The macOS tty buffers ~1 KiB, so reading and parsing on one thread
  stalled the program while it parsed (docs/performance.md).
- **Naming.** Method, action, event and flag names follow `docs/spec.md`, so scripts and agents written
  for that interface carry over. Env vars are `LEON_*`.

## Verification

`cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` (the loop's
`the verify script` runs all three). Terminal behaviour is tested end to end: spawn a real shell in a PTY,
send input, wait for the screen, assert on `capture`. The GUI is checked by window-only screenshots.
