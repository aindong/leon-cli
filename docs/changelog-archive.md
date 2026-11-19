# Changelog: earlier releases

The sections of [CHANGELOG.md](../CHANGELOG.md) for releases before the latest one, newest first,
each as it was written. At each release ([releases.md](releases.md)), the section before the new
one moves whole to the top of this file, so CHANGELOG.md stays under its 10 KB cap: its relative
links are rewritten to start from `docs/` (`docs/x.md` becomes `x.md`, `README.md` becomes
`../README.md`; a `#` anchor stays). `scripts/release-notes.sh` and the site's What's new pages read
a release here when CHANGELOG.md has no section for it, and turn its links back into the repo's.

## [0.1.0] - 2026-11-18

The first release of leon, a terminal multiplexer for people who run several terminal programs
and coding agents at once. A server keeps your sessions, tabs and split panes running, and you
work with them from a native macOS app, the `leon` command in any terminal, Lua, or your coding
agent. Leon.app runs on macOS 11 or later, on Apple Silicon and Intel; the CLI runs on macOS and
Linux. `leon --version` prints `leon 0.1.0 (<commit>)`, and the socket API is `1.12.0`.

Leon.app is not notarized by Apple yet, so macOS won't open the downloaded app until you clear its
quarantine flag: unzip it into `/Applications`, then run
`xattr -dr com.apple.quarantine /Applications/Leon.app`. [Install](../README.md#install) has the
rest.

### What you get

- **Sessions that outlive every window.** A server owns sessions, tabs, split panes and their
  programs. Close the app, quit your terminal or drop your ssh link, and everything keeps running.
  When the server itself stops, the layout, each pane's directory and its scrollback come back on
  the next start, and an agent's resume command (`claude --resume …`) waits at the new prompt.
- **A native macOS app.** Sessions and tabs in a sidebar or the titlebar, split panes with headers,
  floating panes, drawers and popups on a key ([detail](changelog-detail.md#popups)), scrollback, selection, find,
  copy mode, hints, links and mouse support, a command palette, ⌘K to switch to any tab on any
  machine, ⌘⇧\ for every tab as a live card ([detail](changelog-detail.md#the-overview-in-leonapp)),
  and About leon in the leon menu.
- **Make it yours.** Eight themes with a live preview ([detail](changelog-detail.md#themes)), your font
  ([detail](changelog-detail.md#m22b--themes-and-settings-in-the-app)), a translucent window (`translucency`,
  [detail](changelog-detail.md#a-translucent-window-in-leonapp)), an icon for each pane's program
  ([detail](changelog-detail.md#an-icon-per-program-in-leonapp)) and faded unfocused panes (`fade_inactive_panes`,
  [detail](changelog-detail.md#fading-the-panes-without-the-focus)), all in a Settings window (⌘,) with sections
  and search ([detail](changelog-detail.md#a-settings-window-with-sections)).
- **Know which agent needs you.** Programs report working, blocked, done or error (OSC 7501, or
  `leon notify`). The app shows it on tabs and pane headers, lists what needs you under Activity
  (⌘⇧A), badges the Dock, and sends a notification when you're elsewhere.
- **Scriptable from the shell.** `leon run`, `send`, `capture`, `wait`, `split`, `toggle`, `events`
  and the rest, with `--json` and fixed exit codes ([detail](changelog-detail.md#daily-driver-polish)). `leon api`
  reaches every method of a versioned socket API, with a compatibility promise and a reference in
  docs/api.md.
- **Agent-ready.** `leon skill setup` teaches Claude Code and Codex to open panes beside
  themselves, run your tests there and read the results.
- **Lua.** Key bindings, sequences, modes, repeatable keys and your own actions in `init.lua`;
  scripts with `leon do` that react to events; settings, themes and `leon config get/set`.
- **Any terminal, any machine.** `leon attach` is a tmux-like client in any terminal. `-H HOST`
  drives the leon server on another machine over your own ssh, or over Tailscale with
  `serve_tailscale` ([detail](changelog-detail.md#serve-over-tailscale)), and the app shows each host's sessions
  and manages hosts and the server in Settings ([detail](changelog-detail.md#server-and-remote-hosts-in-settings)).
- **Updates you choose.** `leon self-update` installs the newest signed release
  ([docs/updates.md](updates.md), [detail](changelog-detail.md#updates)), and the app looks once a day and
  installs one when you choose Install ([detail](changelog-detail.md#updates-in-leonapp)).
- **Packages.** Leon.app for macOS, a tarball of the CLI for macOS and Linux, and a `.deb`, each
  for arm64 and x86_64, with LICENSE and the third-party notices, and every file signed
  ([detail](changelog-detail.md#releases)).
- **Steady under load.** Fast output ([detail](changelog-detail.md#output-throughput)), panes that open and show
  their output under heavy load ([detail](changelog-detail.md#terminals-under-load)), a font change that resizes
  only the panes on screen ([detail](changelog-detail.md#memory)), and the fixes from an audit of the server and
  the app before the release ([detail](changelog-milestones.md#fixed-in-the-release-audit)).

### The milestones

Each milestone that built 0.1.0, with what it added, changed and fixed, and the fixes from the
daily-driver test and the release audit, are in [docs/changelog-milestones.md](changelog-milestones.md).
