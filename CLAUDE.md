# leon

A Rust + GPUI terminal multiplexer. Read `docs/architecture.md` first (its decisions are in
`docs/architecture/*.md`, kept under 20 KB each; the doc tests read them); leon's own contract is
`docs/api.md` with `docs/api-snapshot.json`, and `docs/roadmap.md` is the milestone order.

- Checks: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` (or `alleo-verify`).
- Linux: `scripts/linux-test.sh` (Docker; the non-app crates, `--workspace --exclude leon-app`), packages
  `scripts/package-linux.sh`. `scripts/test-peer-uid.sh` separately proves peer-uid rejection
  in Docker, including a red run with the check disabled in a disposable source copy. A test gated to macOS needs a `// macOS only: <reason>` line above the gate.
- GPUI needs the `runtime_shaders` feature on this machine (no Metal Toolchain installed).
- Name new methods, actions, events and flags to fit the ones in `docs/api.md` and `docs/lua.md`; env vars are `LEON_*`.
- The API is a promise (`docs/integrators.md#compatibility`): a new method needs a line in
  `crates/leon-server/src/contract.rs`; after changing any method, regenerate `docs/api.md` with
  `LEON_BLESS=1 cargo test -p leon-server --test m18_api_docs`, and a stable schema change also
  needs an `API_VERSION` bump and a CHANGELOG.md entry first.
- Docs are tested: a new or changed command/flag needs `docs/cli.md`, a new app shortcut needs the
  README table, and `sh`/`lua` blocks in the docs must run. Packaging: `scripts/bundle.sh`, `scripts/install.sh`;
  a release: `scripts/release.sh` (every package from a clean clone, with `scripts/notices.sh`'s
  license notices, which need the crates fetched: build first). A tag `vX.Y.Z` publishes one
  (`docs/releases.md`); `latest.json`'s fields are a contract, and the release tests need
  `minisign` and `jq`.
- Terminal tests run a real shell in a PTY; wait for screen text with a timeout, never fixed sleeps.
- Tests that start a server set both `LEON_RUNTIME_DIR` and `LEON_STATE_DIR` to temp dirs (use
  `tests/support::Env`), or they restore and overwrite the real `~/.local/state/leon` sessions.
- GUI checks: screenshot only leon's own window, never the whole screen: `swift scripts/winshot.swift leon out.png`
  (`Leon` for the bundled Leon.app).
  A locked screen draws nothing (macOS occludes the window); `scripts/idle-frames.sh` refuses then.
  Frame stats: `LEON_FRAME_STATS=1 leon app`; headless model cost: `cargo run --release -p leon-app --example frame_bench`.
  Compare two folders of screenshots: `swift scripts/pixel-diff.swift BEFORE AFTER [name:x,y,w,h …]`
  (fails on any changed pixel outside the named boxes, e.g. a pulsing status dot).
- Docs screenshots show a neutral demo, never this machine: retake them with `scripts/demo-shots.sh`
  (temp HOME, neutral prompt and titles, private LEON_* dirs, a fake devbox). No image name, README
  text or picture may show the user's login or hostname; `cargo test -p leon --test m13_privacy`
  checks names, bytes and OCR'd text, and a look at each picture covers the rest.
