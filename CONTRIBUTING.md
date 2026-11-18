# Contributing to leon

leon is a terminal multiplexer for people who run several terminal programs and coding agents at
once: a server keeps sessions, tabs and panes running, and you drive them from a macOS app, the
`leon` command, Lua or your agent. It is written in Rust, with GPUI for the app.

Thanks for helping. This page says where to start, how to set up, and the rules a change has to
follow to land.

## Where to start

- **Good first issues:** [issues labelled `good first issue`](https://github.com/aindong/leon-cli/issues?q=is%3Aissue+is%3Aopen+label%3A%22good+first+issue%22)
  are small and self-contained. Say in the issue that you're taking one.
- **Translations:** the README and the everyday guides can be translated with no Rust at all.
  [docs/i18n/README.md](docs/i18n/README.md) has the steps.
- **Bugs and ideas:** open an issue with the bug report or feature request form. For anything
  bigger than a fix, open an issue first, so we can agree on the approach before you write it.
- **Security problems:** never in a public issue; [SECURITY.md](SECURITY.md) says how to report one.

## Setting up

You need git and a Rust toolchain from [rustup](https://rustup.rs). CI builds with Rust 1.100.0.

On **macOS** (11 or later) the whole workspace builds, the app included. GPUI is built with its
`runtime_shaders` feature, so you don't need Xcode's Metal toolchain. On **Linux** everything
but the app builds (`cargo build -p leon`); the app is macOS only for now.
[docs/install.md](docs/install.md) has the details.

The checks every change passes, the same ones CI runs:

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

The release tests sign with a throwaway key, so they need `minisign` and `jq` on your `PATH`
(`brew install minisign jq`, or your distribution's packages). With Docker you can run the Linux
suite from any machine: `scripts/linux-test.sh` runs everything but the app in the same pinned Rust
image CI uses. The whole suite takes a while; while you work, run the test files that cover your
change (`cargo test -p leon --test <file>`).

## How a change lands

1. Fork the repository and branch from `dev`, the default branch. `main` only moves when a release
   is cut.
2. Commit small, logical steps. Commit messages are plain and lower case, naming the area first:
   `app: …`, `server: …`, `docs: …`, `ci: …`. Say what changed, not how hard it was.
3. Open a pull request into `dev` and fill in the template. CI runs the full gate on macOS and on
   Linux (x86_64 and arm64). A change to English docs alone runs only the doc tests, and a change
   to translations or these community files alone runs nothing.
4. A review bot comments on every pull request; a maintainer reviews and merges.

## House rules

These are the rules reviewers check. Most of them are enforced by tests.

- **Terminal tests run a real shell in a PTY.** Wait for text on the screen with a timeout; never
  sleep for a fixed time.
- **Tests that start a server use private directories:** set both `LEON_RUNTIME_DIR` and
  `LEON_STATE_DIR` to temporary directories (`crates/leon/tests/support` has `Env` for this), or a
  test run would touch your real sessions.
- **A test gated to macOS** says why on the line above the gate: `// macOS only: <reason>`.
- **Docs are tested.** A new or changed command or flag goes in [docs/cli.md](docs/cli.md), a new
  app shortcut in the table in [docs/shortcuts.md](docs/shortcuts.md), and the `sh` and `lua`
  blocks in the docs must run. Links between docs must resolve.
- **The API is a promise** ([docs/integrators.md](docs/integrators.md#compatibility)). A new socket
  method needs a line in `crates/leon-server/src/contract.rs`. After changing any method, regenerate
  [docs/api.md](docs/api.md) with `LEON_BLESS=1 cargo test -p leon-server --test m18_api_docs`; a
  change to a stable schema also bumps `API_VERSION` and gets a [CHANGELOG.md](CHANGELOG.md) entry.
- **Names fit the ones already there:** new methods, actions, events and flags follow
  [docs/api.md](docs/api.md) and [docs/lua.md](docs/lua.md), and environment variables are `LEON_*`.
- **A new feature** adds one summary bullet to [CHANGELOG.md](CHANGELOG.md)'s `[Unreleased]`,
  linking its detail in [docs/changelog-detail.md](docs/changelog-detail.md).
- **App code (GPUI):** read [docs/architecture/gpui.md](docs/architecture/gpui.md) first, and the
  GPUI source for each API you use; don't guess from other versions.
- **Screenshots show a neutral demo, never your machine.** Retake the docs' pictures with
  `scripts/demo-shots.sh`; no picture, file name or text may show your login or host name
  (`cargo test -p leon --test m13_privacy` checks).
- **The app icon is built, never drawn:** [brand/README.md](brand/README.md) has the steps.
- **Nothing private** in code, docs, commits or pictures: no logins, host names, home paths,
  emails or tokens.

[docs/architecture.md](docs/architecture.md) explains how leon is built, and
[docs/roadmap.md](docs/roadmap.md) what comes next.

## Translating

Translations live in `docs/i18n/<lang>/`, one folder per language. [docs/i18n/README.md](docs/i18n/README.md)
lists what is translated, the header each page starts with, what never gets translated, and the
two scripts to run before you open the pull request: `scripts/i18n-check.sh` and
`scripts/i18n-status.sh`.

## Conduct

Everyone taking part in leon follows the [Code of Conduct](CODE_OF_CONDUCT.md).
