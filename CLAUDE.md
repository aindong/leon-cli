# leon

A Rust + GPUI terminal multiplexer. Read `docs/architecture.md` first; `docs/spec.md` is the
interface we match and `docs/roadmap.md` the milestone order.

- Checks: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` (or `the verify script`).
- GPUI needs the `runtime_shaders` feature on this machine (no Metal Toolchain installed).
- Name methods, actions, events and flags as `docs/spec.md` does; env vars are `LEON_*`.
- Terminal tests run a real shell in a PTY; wait for screen text with a timeout, never fixed sleeps.
- GUI checks: screenshot only leon's own window, never the whole screen.
