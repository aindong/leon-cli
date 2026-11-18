# Changelog

Everything that changes in leon, newest first. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the API follows
[Semantic Versioning](https://semver.org/spec/v2.0.0.html) through its `api_version`.

0.1.0 is the first release. Its section sums up what it brings and links the detail of each
feature in [docs/changelog-detail.md](docs/changelog-detail.md); each milestone from
[docs/roadmap.md](docs/roadmap.md) that built it keeps its notes in
[docs/changelog-milestones.md](docs/changelog-milestones.md). Milestones that haven't landed have
no section: the app on Linux (M15's second part; Linux has the server, the CLI and `leon attach`).
`.github/workflows/release.yml` builds and publishes each release ([docs/releases.md](docs/releases.md)).

## Reading `api_version`

`api_version` is the version of the socket API, separate from the build's version. `help.all`
and `server.version` both report it, and `leon api call server.version` prints it. The current
one is `1.13.0`.

- **MAJOR** goes up when a stable method changes in a way that can break a client: a field or a
  method removed or renamed, a type narrowed, a new required parameter, a different meaning.
- **MINOR** goes up when the stable API grows: a new method, or a new optional parameter or result
  field on an existing one.
- **PATCH** goes up for fixes that keep every schema as it was.

Only methods marked `stable` are covered. `experimental` and `internal` methods can change in any
release. A client should check that the major version is the one it was written for and ignore
fields it doesn't know. The full policy is in
[docs/integrators.md](docs/integrators.md#compatibility).

## [Unreleased]

- **Scrollback that costs what its text costs.** A pane keeps its 10,000 lines in about a ninth of
  the memory; `leon.set("scrollback", N)` sets how many, reported by `server.settings` (`api_version`
  `1.13.0`) ([detail](docs/changelog-detail.md#scrollback-that-costs-what-its-text-costs)).

## [0.1.1] - 2026-11-19

A security fix. `leon --version` prints `leon 0.1.1 (<commit>)`; the socket API is still `1.12.0`.

### Security

- With `serve_tailscale` on, the tailnet listener now serves only leon's API and closes any other
  connection, such as a web request, unread. If you turned `serve_tailscale` on, update:
  `leon self-update`, or Install in the app.

### Added

- **Contributing and translations.** A contributor guide, translations of the README and guides in
  `docs/i18n/`, and CI that skips docs-only changes ([detail](docs/changelog-detail.md#contributing-and-translations)).

### Changed

- Docs: the automation, server and agent commands moved from docs/cli.md to pages of their own
  under `docs/cli/`, and the English in the guides is clearer.

## [0.1.0] - 2026-11-18

The first release of leon, a terminal multiplexer for people who run several terminal programs
and coding agents at once. A server keeps your sessions, tabs and split panes running, and you
work with them from a native macOS app, the `leon` command in any terminal, Lua, or your coding
agent. Leon.app runs on macOS 11 or later, on Apple Silicon and Intel; the CLI runs on macOS and
Linux. `leon --version` prints `leon 0.1.0 (<commit>)`, and the socket API is `1.12.0`.

Leon.app is not notarized by Apple yet, so macOS won't open the downloaded app until you clear its
quarantine flag: unzip it into `/Applications`, then run
`xattr -dr com.apple.quarantine /Applications/Leon.app`. [Install](README.md#install) has the
rest.

### What you get

- **Sessions that outlive every window.** A server owns sessions, tabs, split panes and their
  programs. Close the app, quit your terminal or drop your ssh link, and everything keeps running.
  When the server itself stops, the layout, each pane's directory and its scrollback come back on
  the next start, and an agent's resume command (`claude --resume …`) waits at the new prompt.
- **A native macOS app.** Sessions and tabs in a sidebar or the titlebar, split panes with headers,
  floating panes, drawers and popups on a key ([detail](docs/changelog-detail.md#popups)), scrollback, selection, find,
  copy mode, hints, links and mouse support, a command palette, ⌘K to switch to any tab on any
  machine, ⌘⇧\ for every tab as a live card ([detail](docs/changelog-detail.md#the-overview-in-leonapp)),
  and About leon in the leon menu.
- **Make it yours.** Eight themes with a live preview ([detail](docs/changelog-detail.md#themes)), your font
  ([detail](docs/changelog-detail.md#m22b--themes-and-settings-in-the-app)), a translucent window (`translucency`,
  [detail](docs/changelog-detail.md#a-translucent-window-in-leonapp)), an icon for each pane's program
  ([detail](docs/changelog-detail.md#an-icon-per-program-in-leonapp)) and faded unfocused panes (`fade_inactive_panes`,
  [detail](docs/changelog-detail.md#fading-the-panes-without-the-focus)), all in a Settings window (⌘,) with sections
  and search ([detail](docs/changelog-detail.md#a-settings-window-with-sections)).
- **Know which agent needs you.** Programs report working, blocked, done or error (OSC 7501, or
  `leon notify`). The app shows it on tabs and pane headers, lists what needs you under Activity
  (⌘⇧A), badges the Dock, and sends a notification when you're elsewhere.
- **Scriptable from the shell.** `leon run`, `send`, `capture`, `wait`, `split`, `toggle`, `events`
  and the rest, with `--json` and fixed exit codes ([detail](docs/changelog-detail.md#daily-driver-polish)). `leon api`
  reaches every method of a versioned socket API, with a compatibility promise and a reference in
  docs/api.md.
- **Agent-ready.** `leon skill setup` teaches Claude Code and Codex to open panes beside
  themselves, run your tests there and read the results.
- **Lua.** Key bindings, sequences, modes, repeatable keys and your own actions in `init.lua`;
  scripts with `leon do` that react to events; settings, themes and `leon config get/set`.
- **Any terminal, any machine.** `leon attach` is a tmux-like client in any terminal. `-H HOST`
  drives the leon server on another machine over your own ssh, or over Tailscale with
  `serve_tailscale` ([detail](docs/changelog-detail.md#serve-over-tailscale)), and the app shows each host's sessions
  and manages hosts and the server in Settings ([detail](docs/changelog-detail.md#server-and-remote-hosts-in-settings)).
- **Updates you choose.** `leon self-update` installs the newest signed release
  ([docs/updates.md](docs/updates.md), [detail](docs/changelog-detail.md#updates)), and the app looks once a day and
  installs one when you choose Install ([detail](docs/changelog-detail.md#updates-in-leonapp)).
- **Packages.** Leon.app for macOS, a tarball of the CLI for macOS and Linux, and a `.deb`, each
  for arm64 and x86_64, with LICENSE and the third-party notices, and every file signed
  ([detail](docs/changelog-detail.md#releases)).
- **Steady under load.** Fast output ([detail](docs/changelog-detail.md#output-throughput)), panes that open and show
  their output under heavy load ([detail](docs/changelog-detail.md#terminals-under-load)), a font change that resizes
  only the panes on screen ([detail](docs/changelog-detail.md#memory)), and the fixes from an audit of the server and
  the app before the release ([detail](docs/changelog-milestones.md#fixed-in-the-release-audit)).

### The milestones

Each milestone that built 0.1.0, with what it added, changed and fixed, and the fixes from the
daily-driver test and the release audit, are in [docs/changelog-milestones.md](docs/changelog-milestones.md).
