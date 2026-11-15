# Changelog

Everything that changes in leon, newest first. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the API follows
[Semantic Versioning](https://semver.org/spec/v2.0.0.html) through its `api_version`.

0.1.0 is the first release. Its section sums up what it brings; under it, each milestone from
[docs/roadmap.md](docs/roadmap.md) and [docs/research/next-for-leon.md](docs/research/next-for-leon.md)
keeps its own notes, dated by when it landed on `main`. Milestones that haven't landed have no
section: M22b (themes and settings in the app) and the app on Linux (M15's second part; Linux has
the server, the CLI and `leon attach`).

## Reading `api_version`

`api_version` is the version of the socket API, separate from the build's version. `help.all`
and `server.version` both report it, and `leon api call server.version` prints it. The current
one is `1.5.0`.

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

### M22b — Themes and settings in the app

#### Added

- The app draws with init.lua's `theme`, `font.family` and `font.size`: panes, the window around
  them and the status colours. A `{ dark, light }` theme follows the macOS appearance and switches
  with it. Changes apply when the config reloads, without a restart.
- A Settings window (⌘,, "Settings…" in the leon menu and the palette; action `client.settings`):
  the theme for dark and for light mode with a preview of each, font family and size,
  notifications per kind and copy on select. Each change is written to init.lua by
  `leon config set`'s code; a setting set by code it can't rewrite shows read-only with its
  `file:line`.
- `block.watch` and `dev.leon.terminal.lines` take an optional `palette` (`fg`, `bg`, `cursor`,
  `ansi`) that the connection's screens and lines are resolved with; without it they are
  leon-dark's, as before. Both methods are Experimental, so `api_version` is unchanged.

#### Changed

- leon-light: the selected row is a step deeper than a hovered one, and the accent, secondary and
  status colours are a little darker, so all of them read at 4.5:1 on every surface.

## [0.1.0] - not yet tagged

The first release: `leon --version` prints `leon 0.1.0 (<commit>)`. API `1.5.0`.

### What you get

- **Sessions that outlive every window.** A server owns sessions, tabs, split panes and their
  programs. Close the app, quit your terminal or drop your ssh link, and everything keeps running.
  When the server itself stops, the layout, each pane's directory and its scrollback come back on
  the next start, and an agent's resume command (`claude --resume …`) waits at the new prompt
  (`leon.set("resume", "always")` runs it).
- **A native macOS app.** Sessions and tabs in a sidebar or the titlebar, split panes with headers,
  floating panes and drawers, scrollback, selection, find, copy mode, hints, links and mouse
  support, a command palette, and ⌘K to switch to any tab on any machine.
- **Know which agent needs you.** Programs report working, blocked, done or error (OSC 7501, or
  `leon notify`). The app shows it on tabs and pane headers, lists what needs you under Activity
  (⌘⇧A), badges the Dock, and sends a notification when you're elsewhere.
- **Scriptable from the shell.** `leon run`, `send`, `capture`, `wait`, `split`, `events` and the
  rest, with `--json` and fixed exit codes. `leon api` reaches every method of a versioned socket
  API, with a compatibility promise and a reference in docs/api.md.
- **Agent-ready.** `leon skill setup` teaches Claude Code and Codex to open panes beside
  themselves, run your tests there and read the results.
- **Lua.** Key bindings, sequences, modes, repeatable keys and your own actions in `init.lua`;
  scripts with `leon do` that react to events; settings, themes (`leon theme`) and
  `leon config get/set`.
- **Any terminal, any machine.** `leon attach` is a tmux-like client in any terminal. `-H HOST`
  drives the leon server on another machine over your own ssh, and the app shows each host's
  sessions in its sidebar.
- **Packages.** Leon.app for Apple Silicon Macs, and a tarball and a `.deb` for Linux on arm64 and
  x86_64, each with LICENSE and the third-party notices. `scripts/release.sh` builds them all from
  a clean checkout and prints their SHA-256 sums.

### Fixed in the release audit ([docs/audit-0.1.0.md](docs/audit-0.1.0.md))

- A server could open only about 80 panes when it started with a low open-file limit, which is
  what launchd gives the app: past that, new panes failed with "Too many open files". The server
  now raises its own limit, up to 10240.
- On macOS, a second server could unlink the socket of a live server that was too busy to accept
  connections, leaving every program running but out of reach. A server now holds `server.lock`
  for its whole life, and no one unlinks a socket while it is held.
- A window split more than about 60 levels deep, or more than 1024 blocks, saved fine but then
  couldn't be loaded, so the next start set every session aside. A window now nests at most 32
  splits, and a server holds at most 1024 blocks; past either, `session.new_split`,
  `session.move_block` and the like answer 400. Schemas are unchanged.
- Over a host's link, a slow or hostile host could freeze the whole window, local panes too:
  the wheel, scrolling back, find, selections, links and copies waited on the host, and a host
  sending lines that weren't the reply kept a call waiting for ever. `leon attach -H` couldn't read
  its detach key meanwhile. Now nothing on the window's thread waits on a link, and every call has
  one 5 s deadline. A copy from a host's pane copies what the app drew over the range dragged, not
  the host's reply, and a link whose uri isn't the text drawn asks first.
- The app freed nothing when a pane closed or its host went, read every block once per tab for the
  sidebar on each poll, and took whatever counts and layouts a host sent. Remote lists are capped
  (256 sessions, 1024 tabs, panes and layout leaves per session); a split ratio outside 0 to 1 is
  refused. Smaller fixes: a mouse release reports the button let go; a non-UTF-8 line no longer
  drops a server; writes and spawned calls have timeouts. All 14 app findings, with their tests:
  [docs/audit-0.1.0-app.md](docs/audit-0.1.0-app.md).

### Fixed in the [daily-driver test](docs/research/daily-driver-0.1.0.md)

- `leon run`, `split`, `float --new` and Lua find programs on your shell's PATH (login and rc
  files), even when Leon.app started the server from Finder with launchd's bare PATH.
- With `-H`, `run`, `split` and `new` start in the host's home or an absolute `--cwd` there, not
  this machine's directory, and fail within 15 s while the host is unreachable.
- Sessions are saved while the server runs, so a crash no longer restores an older save.
- A resume command is offered at one restart, not at every one after it.
- `leon -H … attach` says when it is reconnecting; its splits keep a remote pane's directory.
- A program not on PATH fails with one line instead of listing every PATH entry.

### Added

- The app's leon menu has **About leon**, showing `leon --version`'s string.
- `leon --version` names the commit: `leon 0.1.0 (1a2b3c4d5e6f)`.
- `LICENSE` and `THIRD-PARTY-NOTICES.txt` (every crate built in, with its license) in Leon.app's
  resources, the Linux tarball and the `.deb`'s docs.
- `scripts/release.sh`: builds every package from a clean checkout of one commit and writes
  `SHA256SUMS`. It never tags, pushes or uploads.

### The milestones

Each milestone that built 0.1.0, with what it added, changed and fixed, is in
[docs/changelog-milestones.md](docs/changelog-milestones.md).
