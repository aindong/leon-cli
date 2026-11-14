# leon

A Rust + GPUI terminal multiplexer. Read `docs/architecture.md` first (its decisions are in
`docs/architecture/*.md`, kept under 20 KB each; a doc test reads all three files); `docs/spec.md` is the
interface we match and `docs/roadmap.md` the milestone order.

- Checks: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` (or `the verify script`).
- Linux: `scripts/linux-test.sh` (Docker; the non-app crates, `--workspace --exclude leon-app`), packages
  `scripts/package-linux.sh`. A test gated to macOS needs a `// macOS only: <reason>` line above the gate.
- GPUI needs the `runtime_shaders` feature on this machine (no Metal Toolchain installed).
- Name methods, actions, events and flags as `docs/spec.md` does; env vars are `LEON_*`.
- Docs are tested: a new or changed command/flag needs `docs/cli.md`, a new app shortcut needs the
  README table, and `sh`/`lua` blocks in the docs must run. Packaging: `scripts/bundle.sh`, `scripts/install.sh`.
- Terminal tests run a real shell in a PTY; wait for screen text with a timeout, never fixed sleeps.
- Tests that start a server set both `LEON_RUNTIME_DIR` and `LEON_STATE_DIR` to temp dirs (use
  `tests/support::Env`), or they restore and overwrite the real `~/.local/state/leon` sessions.
- GUI checks: screenshot only leon's own window, never the whole screen: `swift scripts/winshot.swift leon out.png`
  (`Leon` for the bundled Leon.app).
  A locked screen draws nothing (macOS occludes the window); `scripts/idle-frames.sh` refuses then.
  Frame stats: `LEON_FRAME_STATS=1 leon app`; headless model cost: `cargo run --release -p leon-app --example frame_bench`.
