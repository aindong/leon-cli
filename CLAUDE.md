# leon

A Rust + GPUI terminal multiplexer. Read `docs/architecture.md` first; `docs/spec.md` is the
interface we match and `docs/roadmap.md` the milestone order.

- Checks: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` (or `the verify script`).
- GPUI needs the `runtime_shaders` feature on this machine (no Metal Toolchain installed).
- Name methods, actions, events and flags as `docs/spec.md` does; env vars are `LEON_*`.
- Terminal tests run a real shell in a PTY; wait for screen text with a timeout, never fixed sleeps.
- Tests that start a server set both `LEON_RUNTIME_DIR` and `LEON_STATE_DIR` to temp dirs (use
  `tests/support::Env`), or they restore and overwrite the real `~/.local/state/leon` sessions.
- GUI checks: screenshot only leon's own window, never the whole screen: `swift scripts/winshot.swift leon out.png`.
  A locked screen draws nothing (macOS occludes the window); `scripts/idle-frames.sh` refuses then.
  Frame stats: `LEON_FRAME_STATS=1 leon app`; headless model cost: `cargo run --release -p leon-app --example frame_bench`.
