# Architecture

leon is a terminal multiplexer: a long-lived server owns sessions, windows (tabs), split
layouts and terminal panes ("blocks"); clients (the macOS app, the `leon` CLI, Lua scripts,
agents) drive it through one self-describing API.

## Crates (one Cargo workspace)

| Crate | Job |
|---|---|
| `leon-core` | The object model (server state, sessions, windows, layout tree, blocks), stable IDs and target resolution, terminal blocks (PTY via `portable-pty`, emulation via `alacritty_terminal`), the OSC 7501 program-status parser, and the event log with gapless `seq`. No I/O policy, no sockets. |
| `leon-server` | The API: a method registry where every method carries a description and JSON Schemas (`help.all`), served over a Unix socket (and, when `serve_tailscale` is on, to the owner's other machines over Tailscale). Also runnable in-process. |
| `leon-lua` | Lua 5.1 (`mlua`): `init.lua` config (bindings, sequences, modes, actions), the live config `Runtime`, and `leon do` scripts (`leon.on`, `wait`, timers, `exec`). |
| `leon` (bin) | The CLI: client commands, `leon server run/start/stop/status`, autostart, and `leon attach` (the terminal client). With no arguments it opens the app; a build without the app runs `leon attach` in a terminal and prints the help elsewhere. |
| `leon-hosts` | Other machines: `ssh://` and `tailnet://` endpoints, the hosts list, the `Link` that reaches a host's server over ssh or directly over Tailscale, and tailscaled's read-only LocalAPI. No GPUI, so the CLI and (later) the app share it. |
| `leon-update` | The updater: fetches the signed release manifest, verifies it and the release file against the keys built in, and swaps the new leon in atomically. `leon self-update` calls it, and so does Leon.app (`leon-app/src/updates.rs`, docs/updates.md). |
| `leon-app` | The GPUI client (macOS for now). It attaches to the server over the socket like any client and paints the screens the server pushes, so programs outlive the app. |

## Platforms

| | macOS | Linux (x86_64, aarch64) |
|---|---|---|
| Server, CLI, Lua, agent skill | yes | yes |
| The app (`leon app`, `leon` alone) | yes | not yet (M15 part 2): `leon` alone is `leon attach` in a terminal, else the help; `leon app` exits 1 |
| Tested | everything, `cargo test` | everything but the app, `scripts/linux-test.sh` in Docker |
| Packages | `Leon.app` (`scripts/bundle.sh`, `scripts/install.sh`) | tarball and `.deb` (`scripts/package-linux.sh`) |

The app is the `leon` crate's `app` feature, on by default, which turns on `leon-app`'s `gui`
feature; gpui is a macOS-only dependency of `leon-app`, and the code that needs it is behind
`cfg(all(feature = "app"|"gui", target_os = "macos"))`. Without `gui`, `leon-app` is the GPUI-free
part `leon attach` shares on every platform: the client (`remote`), keymap, palette and pane layout.
Cargo has no per-platform default features, so the pair is the per-platform default: `cargo build`
gives the app on macOS and a headless binary on Linux with no flags, and `--no-default-features`
gives a headless macOS build. `leon-app`'s own tests need the window, so Linux commands pass
`--workspace --exclude leon-app`. Tests of the app and of `Leon.app` are gated to macOS with a
`// macOS only: <reason>` line, which `scripts/linux-test.sh` requires and lists.

Platform code: the socket goes under `$XDG_RUNTIME_DIR/leon` when that is set (Linux desktops set
it; the dir must still be private); the peer check is tokio's `peer_cred` (`SO_PEERCRED` on Linux,
`getpeereid` on macOS); a program's cwd is `/proc/<pid>/cwd` on Linux (a removed dir, which reads
`<path> (deleted)`, is dropped for the next fallback) and `proc_pidinfo` on macOS; the skill hook
reads `/proc/<pid>/stat` for the process tree there. `leon server proxy` and the `Link` copy
between a socket and ssh's pipes with `leon_hosts::pump`, a plain read/write loop: `std::io::copy`
splices on Linux, and a splice from a socket into a pipe holds the pipe locked while it waits, so
sshd couldn't read the proxy's stdout and stopped forwarding the request. Signals, process groups,
`flock`, PTYs (`portable-pty`) and the state dir are the same POSIX code on both.

`scripts/linux-test.sh [--arch arm64|amd64]` runs clippy, `cargo test --no-fail-fast` and
`scripts/linux-smoke.sh` (start, a session, `run -- echo hi`, `capture`, a block's cwd, stop, then
the socket through `XDG_RUNTIME_DIR`) for the non-app crates in the pinned official image
`rust:1.100.0-bookworm` (tag and digest in `scripts/linux-image.sh`), as your own uid (1000 when
you're root) rather than root, so the private-dir and peer checks are real and what it writes to
the repo stays yours, with the cargo registry in the named volume `leon-cargo-registry` and each
arch's target dir in `leon-linux-target-<arch>`. The source is
mounted read-only. CI (`.github/workflows/ci.yml`) runs the full macOS gate and the Linux script
and packages on x86_64 and aarch64 runners. On an Apple Silicon Mac `--arch amd64` runs under
Rosetta, which leaks its own fds into children (tests that spawn from several threads deadlock on
each other's pipes: pass `--test-threads=1`) and writes `$HOME/.cache/rosetta` (the two
`m7a_skill` tests that check nothing was written then fail); native x86_64 has neither.

## Decisions

The decisions, in nine parts:

- [Protocol and clients](architecture/protocol-and-clients.md): the wire protocol, events, layout,
  the socket path, terminal emulation, Lua (config, runtime, bounds), the app, the screen transport,
  scrollback, selection, find, links and mouse reporting, `leon attach`, the app keymap and look,
  and the agent skill.
- [Scrollback](architecture/scrollback.md): alacritty's rows and leon's store behind them: when
  rows move, why none is lost or renumbered, styles, clears, resize and the cap.
- [Switching and attention](architecture/attention.md): the session switcher, Activity, the
  badges, notifications, the fade of the panes without the focus, the translucent window, and
  `leon notify`.
- [Repeat keys, copy mode and hints](architecture/keys-copy-hints.md): repeatable bindings in the
  app and `leon attach`, the app's copy mode and hints, and why `leon attach` has no copy mode.
- [Floats and drawers in the app](architecture/floats.md): the shared client's overlays, where
  floats and drawers go, dragging them through the server, slides and Reduce Motion.
- [State, packaging and hosts](architecture/state-and-hosts.md): persistence, scrollback and resume
  after a restart, terminal reads, packaging (Leon.app, Linux packages), the login shell's PATH,
  `LEON_BIN_DIR`, the API contract, tested docs, and naming.
- [Hosts](architecture/hosts.md): remote hosts and reconnects, and the app's hosts.
- [Serving over Tailscale](architecture/tailnet.md): the server's one network listener, who it
  lets in, `tailnet://` endpoints, and the threat model.
- [The web client](architecture/web.md): the page served over the tailnet listener with
  `serve_web`, the WebSocket that carries the socket's API, and how other sites are kept out.
- [How leon uses GPUI](architecture/gpui.md): the app's view structure, redraws, animation,
  painting, input, async and tests as GPUI intends them, what may go beside GPUI, and that GPUI
  is used as published.
- [Cursor shaders](architecture/cursor-shaders.md): users' Shadertoy-style cursor shaders on an
  overlay of leon's own above GPUI (an amendment to the GPUI rules), and its Linux plan.

## Verification

`cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`; on Linux, `scripts/linux-test.sh` (Platforms, above). Terminal
behaviour is tested end to end: spawn a real shell in a PTY, send input, wait for the screen, assert
on `capture`. The GUI is checked by window-only screenshots.
