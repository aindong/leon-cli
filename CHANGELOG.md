# Changelog

Everything that changes in leon, newest first. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the API follows
[Semantic Versioning](https://semver.org/spec/v2.0.0.html) through its `api_version`.

0.1.0 is the first release. Its section sums up what it brings; under it, each milestone from
[docs/roadmap.md](docs/roadmap.md) keeps its own notes, dated by when it landed on `main`.
Milestones that haven't landed have no section: the app on Linux (M15's second part; Linux has
the server, the CLI and `leon attach`).

## Reading `api_version`

`api_version` is the version of the socket API, separate from the build's version. `help.all`
and `server.version` both report it, and `leon api call server.version` prints it. The current
one is `1.9.0`.

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

### An icon per program in Leon.app

#### Added

- Each pane's header, tab, sidebar row and overview card shows an icon for the kind of program in
  it: shell, agent, editor, server, test run, monitor, remote or other
  ([docs/architecture/attention.md](docs/architecture/attention.md)).
- `session.list_blocks` gives each block `foreground`, the name of the program in front
  (never its arguments; null for another user's), and a change is a `foreground_changed` event: a
  new result field on a stable method, so `api_version` goes from `1.8.0` to `1.9.0`.

### A translucent window in Leon.app

#### Added

- Leon.app's window shows a blur of what is behind it, tinted by the theme: the sidebar, the
  title bar and the gaps between panes let the most through, the panes less. Text keeps its
  contrast over any desktop, light or dark: where the setting asks for more than that allows,
  the app draws that part more opaque. Reduce Transparency (System Settings › Accessibility ›
  Display) and full screen draw the window opaque.
- `leon.set("translucency", N)` sets how much shows through, from 0 (opaque, the window as
  before) to 0.8. `server.settings` reports `translucency`, a new result field on a stable
  method, so `api_version` goes from `1.7.0` to `1.8.0`.

### Themes

#### Added

- Six new built-in themes made for leon from its brand palette, beside leon-dark and leon-light:
  leon-maroon (a warm dark), leon-tide (a cool dark), leon-coal (a high-contrast dark), leon-linen
  (a paper light), leon-sand (a soft light) and leon-mist (a cool light). Every built-in theme is
  tested to keep body and UI text at 4.5:1 or better, its ANSI colours readable, selection and
  find marks visible, and the four status colours apart. `leon theme list` and the Settings
  window list them.
- Change Theme… in the command palette (⌘⇧T) lists every theme, built in and imported, and draws
  the window in the selected one as you move through the list; ↩ keeps it in init.lua and esc
  goes back.
- A theme change fades in over a moment instead of switching at once; with Reduce Motion it still
  switches at once.

#### Changed

- A theme imported under one of the new built-in names (`leon-tide`, say) is now shadowed by the
  built-in one; import it again under another `--name` to keep using it.

### Terminals under load

#### Fixed

- A program that writes and exits at once (`printf`, `pwd`) no longer leaves an empty pane when the
  machine is busy. On macOS a program's exit waits only about 0.6 s for its output to be read,
  then drops it; leon now keeps the PTY open until the program has exited, so the exit waits for
  leon to read it.
- A program's exit is reported after the output already read from it is on the screen, also when
  the end of its output is slow to come.
- Opening a pane no longer fails now and then with "failed to openpty" while other programs open
  and close terminals: macOS's "open it again" answer is now followed.

### The overview in Leon.app

#### Added

- ⌘⇧\ (or the toolbar's grid button, or "Overview of All Tabs" in the palette) fills the window
  with every tab of every session and host as a card: its title, its status in the sidebar's
  colours and Activity's words, and a live, scaled-down view of its panes as laid out. Arrow keys move
  between cards, ↩ or a click goes to the tab, typing filters by title, and esc goes back to where
  you were. ⌘\ shows only the current session's tabs. A host that isn't connected shows its last
  cards greyed, with the reason.
- Resting the pointer on a tab in the titlebar, or on a tab's row in the sidebar, shows a live
  thumbnail of that tab; moving to the next tab moves it at once.
- Thumbnails are drawn from the screens leon already has, never captured from the window: a card
  draws again only when its screen changes, at most four times a second, and an idle overview
  draws nothing.

### Output throughput

#### Changed

- A big `cat` of text and colour is 44% faster (80 to 115 MB/s on the perf harness's 100 MB
  file): leon's own scans of the output skip what they don't follow, and the screen push no longer
  holds the terminal's lock while it builds a frame (docs/performance-cat.md).

### Updates in Leon.app

#### Added

- Leon.app looks for a newer release once a day, in the background, and shows a quiet notice in
  the window when one exists: its version, its release notes, and Install. Nothing is downloaded
  or replaced until you choose Install (or "Install Update" in the command palette); the notice
  then shows each step, and when it is done, Restart opens the new version with your sessions as
  they were. Dismissing the notice hides it until a newer version comes out. "Check for Updates"
  in the palette looks at once. It checks and installs what `leon self-update` does
  ([docs/updates.md](docs/updates.md)).
- A Leon.app that Homebrew installed shows the `brew upgrade --cask` command to copy instead of
  Install. An app run from where macOS put a download (App Translocation) says to move it to
  Applications first, and an install that macOS's App Management setting refuses says where to
  allow it.
- `leon.set("check_for_updates", false)` turns the daily look off: the app then makes no request
  of its own. `server.settings` reports `check_for_updates`, a new result field on a stable
  method, so `api_version` goes from `1.6.0` to `1.7.0`.

#### Security

- A release server elsewhere can no longer redirect an update to this machine over https either
  (`https://127.0.0.1`, `[::1]` with or without a zone, `localhost`, `127.1`, a %-escaped name
  and the like); before, only plain http to a loopback address was refused. An address that is
  not ASCII is refused outright. What is downloaded was, and is, verified against the release key.

### Memory

#### Changed

- A font change in Leon.app (⌘=, ⌘-, ⌘0, `font.size` or `font.family` in init.lua) now
  fits only the panes on screen. A pane in another tab or session, a hidden float or drawer, or a pane a
  zoom covers keeps its size until it is shown, and is fitted then, as after a window resize.
  Resizing a pane reallocates its whole history at the new width, so before, one font change
  could grow the server by hundreds of MiB for panes nobody was looking at. A program in a hidden
  pane gets its resize when the pane is shown; `capture` and `dev.leon.terminal.size` report the
  old size until then. A pane that only `leon attach` shows keeps the size attach gave it. ⌘0 at
  the size the font already has, or an init.lua reload that leaves the font, resizes nothing.

### Fading the panes without the focus

#### Added

- Leon.app fades every pane but the focused one toward the window's background (its text, its
  background and its header), so the eye rests on the focused pane. When the focus moves, the
  pane losing it fades out and the pane gaining it fades in over 120 ms; with Reduce Motion the
  change is immediate. A pane alone in its tab is not faded.
- The fade never hides what you are waiting for: a pane's status indicator keeps its colour, and
  a pane that needs you (blocked, an error, a finish you haven't looked at) is not faded at all.
- `leon.set("fade_inactive_panes", N)` sets how strong the fade is, from 0 to 0.8 (the default
  is 0.35, where body text keeps a contrast of 4.5:1 on leon-dark and leon-light); 0 turns it
  off. `leon config get` and `leon config set` take it, and a value out of range is refused.
- `server.settings` reports `fade_inactive_panes`. A new result field on a stable method raises
  `api_version` from `1.5.0` to `1.6.0`.

### Updates

#### Added

- `leon self-update` installs the newest release: it verifies the signed manifest against the
  keys built into leon, takes a release only if it is newer, checks the file's sha256 and
  signature, and swaps the new leon in atomically. `--check` only reports, and exits 10 when a
  newer release exists. A leon installed by Homebrew or from a `.deb` is told the command that
  updates it instead. leon never checks on its own. `LEON_RELEASE_URL` points it at a mirror.
  [docs/updates.md](docs/updates.md) has the checks and what each refusal means.

#### Fixed

- A pane whose program can't be executed is refused like one that isn't there: `leon run`,
  `block.create` and `session.create` fail with `bad_request`, name the program and the OS's
  reason ("can't run its interpreter /no/such/interp: No such file or directory" for a script
  whose `#!` interpreter is missing, "Exec format error" for a file that isn't a program), and
  leave no block behind. Before, the pane showed `fatal runtime error: assertion failed` and
  the program's exit was 134 (or a binary ran as a `/bin/sh` script). A script without a `#!`
  line still runs with `/bin/sh`.

### Releases

#### Added

- A release workflow (`.github/workflows/release.yml`): a tag `vX.Y.Z` builds the app for macOS
  on arm64 and x86_64, the CLI tarballs, and the Linux tarballs and `.deb`s, signs every file with
  minisign, and publishes a GitHub Release with a manifest (`latest.json`) at a stable address.
  Started by hand it stops at a draft. [docs/releases.md](docs/releases.md) has the manifest
  format, the secrets and key rotation.
- `leon-<version>-<arch>-macos.tar.gz`: the CLI alone for macOS, from `scripts/release.sh`.
- A Homebrew tap, `aindong/homebrew-leon`, with a cask for Leon.app and a formula for the CLI,
  rewritten by each release.
- `scripts/release-dry-run.sh` builds, signs and checks a release locally with a throwaway key;
  `scripts/release-verify.sh` checks any release's sums, manifest and signatures.

### Daily-driver polish

#### Added

- `leon run --wait` prints the program's text after the block ID once it exits (what
  `leon capture --trim` shows) when stdout is not a terminal, so an agent gets the output and the
  status in one call. `--output[=true|false]` turns it on or off. The block stays open until it
  has been read, then closes (unless `--keep-open`). (F5)
- `leon wait --text TEXT` waits until a block's screen or scrollback shows `TEXT`: a prompt, a
  "ready" line. It exits 1 after `--for` or if the block closes first. (F6)

#### Changed

- `leon skill setup` without `--agent` also sets up Claude Code or Codex when `claude` or `codex` is
  on PATH, even before its first run, and says for each agent why it was picked. With none found
  it says so, and to pass `--agent`. An agent home dir it creates is private (0700). (F4)
- The README's first commands, the agent guide and docs/agents.md wait for Python's prompt before
  typing into it, and for its answer before reading it; a test runs the README's block. (F6)
- The scratch drawer (⌘J, `leon toggle scratch --create`, `session.toggle` with `create`) opens in
  the directory of the active window's focused block instead of `~`, when its program is still
  running (an exited one's directory is only what its output last said). (F14)

#### Fixed

- A one-shot `claude -p` (`--print`, or `-p` among other short flags, `-cp`) in a pane no longer
  registers a resume command. (F10)
- Keys typed into `leon -H … attach` while the link is stalled no longer arrive in part (the
  research run left `ec` of `echo …⏎` at the remote prompt). After a quiet second, keys wait for
  a probe of the link; if it fails they are all dropped, and so are keys typed while it
  reconnects, and the status bar says how many were not sent. A paste waits in the same line, so
  it can't pass keys typed before it. (F12)
- On macOS, opening a host (`-H`, the app's hosts) could wait 30 s and then fail with `the leon
  server can't be reached: Resource temporarily unavailable`. The ssh started for one of a
  link's connections could inherit the pipes of another that was starting at the same moment,
  which then was never served. ssh now gets its stdin, stdout and stderr and nothing else.
- A clicked notification wakes every window, not only the one opened last.
- A program started by `leon.exec` gets its stdin, stdout and stderr and nothing else: it could
  hold the terminal of a pane that was opening at the same moment, which then never saw its own
  program exit.

No schema changed, so `api_version` stays `1.5.0`.

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

### Fixed in the release audit

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
  drops a server; writes and spawned calls have timeouts. All 14 app findings have their own tests
  (`crates/*/tests/hard_*.rs`).

### Added

- The app's leon menu has **About leon**, showing `leon --version`'s string.
- `leon --version` names the commit: `leon 0.1.0 (1a2b3c4d5e6f)`.
- `LICENSE` and `THIRD-PARTY-NOTICES.txt` (every crate built in, with its license) in Leon.app's
  resources, the Linux tarball and the `.deb`'s docs.
- `scripts/release.sh`: builds every package from a clean checkout of one commit and writes
  `SHA256SUMS`. It never tags, pushes or uploads.

### The milestones

Each milestone that built 0.1.0, with what it added, changed and fixed, and the fixes from the
daily-driver test are in [docs/changelog-milestones.md](docs/changelog-milestones.md).
