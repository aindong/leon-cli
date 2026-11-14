# Architecture

leon is a terminal multiplexer: a long-lived server owns sessions, windows (tabs), split
layouts and terminal panes ("blocks"); clients (the macOS app, the `leon` CLI, Lua scripts,
agents) drive it through one self-describing API. `docs/spec.md` is the interface we match.

## Crates (one Cargo workspace)

| Crate | Job |
|---|---|
| `leon-core` | The object model (server state, sessions, windows, layout tree, blocks), stable IDs and target resolution, terminal blocks (PTY via `portable-pty`, emulation via `alacritty_terminal`), the OSC 7501 program-status parser, and the event log with gapless `seq`. No I/O policy, no sockets. |
| `leon-server` | The API: a method registry where every method carries a description and JSON Schemas (`help.all`), served over a Unix socket. Also runnable in-process. |
| `leon-lua` | Lua 5.1 (`mlua`): `init.lua` config (bindings, sequences, modes, actions) and `leon do` scripts (`leon.on`, `wait`, timers, `exec`). |
| `leon` (bin) | The CLI: client commands, `leon server run/start/stop/status`, autostart. With no arguments it opens the app. |
| `leon-app` | The GPUI client. It embeds the server in-process (no socket hop for rendering) and still listens on the socket, so the CLI and agents drive the same state. |

## Decisions

- **Wire protocol.** Newline-delimited JSON over a Unix socket. Request `{"id","method","params"}`,
  response `{"id","result"}` or `{"id","error":{"status","code","message"}}`. Events are pushed on
  the same connection as `{"event":{...},"seq":N}` after a subscribe call. The spec leaves framing
  unspecified; NDJSON is the simplest thing that streams and is easy to debug with `nc -U`.
- **Socket path.** `$LEON_RUNTIME_DIR/server.sock`, else `$XDG_RUNTIME_DIR/leon/server.sock`, else
  `~/.local/state/leon/run/server.sock`. Mode 0600.
- **Terminal emulation.** `alacritty_terminal` (battle-tested, used by Zed). leon scans the PTY byte
  stream for OSC 7501 itself before handing bytes to the emulator, since unknown OSCs are dropped.
- **GPUI build.** `gpui` 0.2.2 with the `runtime_shaders` feature, so building does not need Xcode's
  separate Metal Toolchain download.
- **Async.** `tokio` in the server; PTY readers on their own threads feeding the emulator.
- **Naming.** Method, action, event and flag names follow `docs/spec.md`, so scripts and agents written
  for that interface carry over. Env vars are `LEON_*`.

## Verification

`cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` (the loop's
`the verify script` runs all three). Terminal behaviour is tested end to end: spawn a real shell in a PTY,
send input, wait for the screen, assert on `capture`. The GUI is checked by window-only screenshots.
