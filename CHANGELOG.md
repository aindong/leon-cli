# Changelog

Everything that changes in leon, newest first. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the API follows
[Semantic Versioning](https://semver.org/spec/v2.0.0.html) through its `api_version`.

leon has had no release yet: every build so far is version 0.1.0. Until the first release, each
section is a milestone from [docs/roadmap.md](docs/roadmap.md) and
[docs/research/next-for-leon.md](docs/research/next-for-leon.md), dated by when it landed on
`main`. Milestones that haven't landed have no section: M15 (Linux) is still to come.

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

### Joined panes, their events, long socket paths (API `1.5.0`)

#### Fixed

- A command run in a pane that joined another session acts on the session that holds its
  `LEON_BLOCK` now, not the `LEON_SESSION` it started with: `leon run`, `leon split`, a short
  `-b` name, `leon events`, scripts, and the agent hook's resume registration. `LEON_SESSION` is
  the fallback when `LEON_BLOCK` isn't set or is gone.
- A pane joining another session (`session.move_block`) sends `block_closed` to the session it
  leaves and `block_created` to the one it joins, so each session's subscribers (`leon events -s`,
  attach) hear it, as for a kill and a create. This raises `api_version` from `1.4.0` to `1.5.0`.
- A socket path too long for this system (104 bytes on macOS, 108 on Linux, the NUL included)
  fails at once: `leon server start` and a command that would start a server name the path, its
  length and the limit, and say to set a shorter `LEON_RUNTIME_DIR`. It used to say only "the
  server didn't come up".

### Loose ends after M12–M21 (API `1.4.0`)

#### Changed

- `session.move_block` takes a block from any session: one from another session joins the
  anchor's session, beside the anchor (tmux's `join-pane`); a session it empties closes. The
  moved program keeps the `LEON_SESSION` it started with. A window a pane joins from another
  window or session unzooms, so the pane shows. This raises `api_version` from `1.3.0`
  to `1.4.0`.
- ⌘↩ in the session switcher puts a pane of another session on this machine in a split beside
  the focused one; on a row of another machine it says why it can't, and ↩ still goes there.
  With a floating or docked pane focused, the pane joins beside a tiled one. A notice clicked after
  its pane joined another session goes to the pane where it is now.

#### Fixed

- The find bar's count ends in `+` (`n of N+`) and copy mode's badge says `N+ matches` when find
  stopped short of every match (10000, or 300 ms).
- Copy mode holds the view still while output arrives, also with its cursor on the bottom row
  (tmux); it no longer ends when a resize shortens the screen under its cursor.
- `scripts/linux-test.sh` runs as a user named `builder`, so the privacy test no longer takes the
  project's name in the README for the login.

### M22a — Themes and settings, the model half (API `1.3.0`)

#### Added

- A theme model outside the app (`leon_core::theme`): the terminal palette, the app's chrome
  and the status colours. `leon-dark` is the app's colours unchanged; `leon-light` is new, Iron on
  Parchment, with every text and status colour at 4.5:1 or more.
- `leon theme list`, `leon theme import FILE [--name NAME]` (Ghostty theme files and iTerm2
  `.itermcolors`, stored in `themes/` beside init.lua) and `leon theme show NAME`.
- Settings in init.lua: `leon.set` takes `theme` (a name, or `{dark, light}` to follow the OS),
  `font.family`, `font.size`, `notifications.done|blocked|error`, as well as `copy_on_select`,
  `repeat_time`, `hint_patterns` and `resume`. `leon config get [KEY]` reads them; `leon config set KEY VALUE` edits init.lua in
  place, keeping comments, and refuses a key set by code it can't rewrite.
- `server.settings` (stable): every effective setting and where each was set, and the
  `settings_changed` event when a reload changes them. This additive change raises `api_version`
  from `1.2.0` to `1.3.0`.

#### Changed

- The notification switches are `notifications.blocked`, `notifications.error` and
  `notifications.done` (M13 shipped them unreleased as `notify_*`; those names are gone), and
  `client.keymap`'s settings carry them as `notifications`.

The app doesn't use the themes, fonts or `leon config set` yet; that is M22b.

### M19: tmux migrants

#### Added

- Repeatable bindings: `leon.bind(key, target, args, { repeats = true })`, like tmux's `bind -r`,
  in the app and `leon attach`, with `leon.set("repeat_time", ms)` (default 500).
- Copy mode in the app (⌘⇧C, `client.copy_mode`): vi keys over the scrollback, `v`/`V` selections,
  `/` and `?` search with `n`/`N`, `y` copies.
- Hints in the app (⌘⇧H, `client.hints`): labels on the URLs, paths, git hashes, UUIDs, hex and IP
  addresses and long numbers on screen; a label copies, in upper case it types the text into the
  pane. `leon.set("hint_patterns", {...})` adds patterns.
- docs/lua.md: a tmux-style preset and a vim-navigator recipe.
- `repeats` on `client.keymap` bindings and `repeat_time` and `hint_patterns` in its settings
  (`internal`).

#### Changed

- `leon actions` lists the app's own actions after the config's.
- Shifted symbols (`&`, `|`, `%`, `"`, …) in bindings are documented as accepted as typed.

### M12 + M13: switch fast, and know which agent needs you

#### Added

- The session switcher (⌘K, `session.switch`): every tab on every host, and a block when the query
  matches its title, with its status; the most recently used first. ↩ goes there; ⌘↩ moves the
  pane beside the focused one when both are in the same session.
- Activity (⌘⇧A, `client.sidebar.activity.toggle`, a toolbar button): every block that needs you
  across hosts, blocked (with its kind), error, done-unseen, then working with its progress, each
  with how long it has waited; a click goes to the pane, which clears its done or error.
- The blocked count on the Activity button and the Dock icon, shown only when something is blocked.
- Notifications from Leon.app (UNUserNotificationCenter) when a block blocks or errors, or a run of
  30 s or more finishes while the app is in the background; once a minute per block and state, four
  a minute in all; `leon.set("notifications.blocked" | "notifications.error" |
  "notifications.done", false)` turns a kind off. Clicking one goes to its pane.
- `leon notify [-b BLOCK] [--state working|blocked|done|error] [--title T] [--body B]` and the
  experimental block method `dev.leon.terminal.report_status`: a status for your own block, held to
  the OSC 7501 rules.

#### Changed

- The app and `leon attach` read at most 16 MiB per line from a server (was 256 MiB). The server
  splits a screen update past 4 MiB into several lines and trims `dev.leon.terminal.lines` replies
  to 4 MiB of rows (ask again for the rest).
- Applying a screen update is linear in its size, whatever a server sends; a line given twice
  takes its last row, and a row keeps at most a line's worth of spans, text and backgrounds.
- A line past the cap ends the connection instead of re-attaching to it; event and screen lines
  are parsed into their types, so a server gets about a byte of the app's memory per byte it sends.
  Names and status text from a server are stripped of hidden characters before they are shown.
- The docs' screenshots are retaken with a neutral demo (`scripts/demo-shots.sh`), and a test keeps
  the user's login and machine name out of them.

### M20a — Floating panes and drawers (API `1.2.0`)

Add session.float_block, new_floating, dock_block, unfloat_block and toggle. Window views now carry floating (back to front) and docked placements, including hidden panes; hiding preserves programs. CLI/Lua and attach support floats and drawers.

### M21: projects and worktrees as a recipe

#### Added

- `leon new --cwd DIR` (also `leon session new`): caller-relative or absolute directories;
  missing paths and files are usage errors (exit 2).
- Optional `cwd` on `session.create`, overriding the first terminal's layout cwd. This additive
  change raises `api_version` from `1.0.0` to `1.1.0`.
- Example Lua action and script for project/worktree sessions, with an opt-in ignored-file hook;
  see [the recipe](docs/recipes.md).

#### Changed

- The spec and the reference app research now have short indexes linking their original sections.

### M18: a promise for integrators

#### Added

- `api_version` (`1.0.0`) in `help.all` and `server.version`.
- A stability marker on every method and block method (`stable`, `experimental` or `internal`),
  shown by `leon api list` and `leon api describe`, which now also list the block methods.
- A `deprecated` field (`since`, `replacement`, `removal`) for methods on their way out. None is
  deprecated yet.
- Request and response examples on the most-used methods (`request_example`, `response_example`).
- [docs/api.md](docs/api.md), the API reference generated from `help.all`; a test fails when it
  drifts from the server.
- [docs/api-snapshot.json](docs/api-snapshot.json), the stable methods' schemas; a test fails on any
  change to them until the snapshot is updated after a version bump.
- [docs/integrators.md](docs/integrators.md): how to write a client in any language, with a
  tested Python client.
- This changelog.

## M17: `leon attach`, the terminal client - 2026-11-15

### Added

- `leon attach [TARGET]`: a terminal UI client that runs in any terminal, locally or over ssh. It
  picks a session with a preview, shows tabs and split panes, and detaches with `ctrl+b d`.
- tmux-style keys after a prefix (`ctrl+b` by default, `--prefix` or the config's `attach` mode to
  change it), with config bindings and sequences on top.
- The host terminal is restored on detach, errors, panics, signals, and even when the client is
  killed (a watchdog process).

## M16b: hosts in the app - 2026-11-15

### Added

- The app's sidebar shows This Mac and every host from `leon hosts`, each with its sessions and a
  dot for its state: connecting, connected, reconnecting, unreachable (with ssh's own words), or
  needs login (with the advice to run `leon hosts check`).
- New Session on any connected machine and Add Host… in the command palette; splits, keys and new
  tabs act on the focused pane's host.

### Changed

- The app and `leon attach -H` resume their event stream after a dropped link instead of attaching
  afresh, and notice a half-dead link in about 15 s.

## M16a: remote sessions over SSH - 2026-11-15

### Added

- `-H HOST` on every command: reach a leon server on another machine through the system `ssh`,
  with no forwarding and nothing listening on TCP.
- `leon hosts` to keep a list of named hosts, and `leon server proxy`, which bridges ssh to the
  remote server's socket (starting it if needed).
- Resumable event streams: `session.attach`/`session.attach_all` with `resumable: true`, and
  `session.resume` to pick up after the last `seq` seen. `leon events -H` and `leon attach -H`
  reconnect on their own when the link drops.

## M14: agents come back after a restart - 2026-11-15

### Added

- A block method, `dev.leon.terminal.set_resume`, that registers the command that resumes a
  program. The agent skill's hook records `claude --resume …` or `codex resume …` with it.
- The `resume` setting (`ask`, `always`, `never`): restored blocks type the resume command at the
  new prompt, and with `always` press Enter too. Nothing else that ran is run again.
- Each block's scrollback is saved (capped, control characters removed) and shown again after a
  restart, dimmed, above a "restored from" line.

## M11: scrollback, selection, copy, mouse and find - 2026-11-15

### Added

- Scrolling through scrollback with the wheel and trackpad, click-drag, double- and triple-click
  selection, and copy with Cmd+C.
- Mouse reporting to programs that ask for it, with Shift held to select anyway.
- A find bar (Cmd+F) over scrollback and screen, and clickable links, OSC 8 included.
- Block methods for clients: `lines` (history by absolute line number), `find`, `selection` and
  `link_at`.

## M10b: the UI pass - 2026-11-15

### Changed

- Tabs live in one place: as rows in the sidebar by default, or in the titlebar
  (`client.tab_style.toggle`), remembered across launches.
- leon's brand palette everywhere, inset pane tiles with headers and controls, one toolbar row,
  titles set by programs, an empty state that teaches, and one icon set.

## M10: the app attaches - 2026-11-14

### Changed

- The app connects to the running server, or starts one, instead of owning it. Quitting the app
  leaves every program running; "Quit and End All Sessions…" or `leon server stop` ends them.

### Added

- The screen transport: `block.watch` pushes screens over the socket, and
  `dev.leon.terminal.screen` returns one whole.

## M9: app polish - 2026-11-14

### Added

- Program status (OSC 7501) in tabs, the sidebar and panes.
- A command palette, the Lua keymap in the app, new panes in the current directory, and font size
  keys.

## M8: persistence and speed - 2026-11-14

### Added

- The server saves its sessions when it stops and restores them when it starts: windows, layouts,
  labels and focus come back with the same IDs. Programs start fresh; saved commands are never
  re-run.
- Measured input latency and throughput, with a benchmark in the repo
  ([docs/performance.md](docs/performance.md)).

## M7: agents and shipping - 2026-11-14

### Added

- `leon skill setup|status|remove|show` for Claude Code and Codex (M7a).
- `Leon.app` (`scripts/bundle.sh`), an install script (`scripts/install.sh`), tested docs, and
  `LEON_BIN_DIR` so `leon` resolves inside the app's terminals (M7b).

## M6: the app - 2026-11-14

### Added

- The GPUI app: a window with sessions in a sidebar, tabs, and live terminals, with keyboard input
  (M6a).
- Split panes with dividers you can drag, click to focus, zoom, and keys to split and move focus
  (M6b).

## M5: Lua - 2026-11-14

### Added

- `init.lua` config: key bindings, sequences, modes and actions, checked by `leon config check`
  (M5a).
- A live config the server keeps: `leon do` scripts with `leon.on`, `wait`, timers and `exec`,
  `leon actions`, `leon keymap` and `leon config reload` (M5b).

## M4: program status - 2026-11-14

### Added

- OSC 7501 program status: programs report working, done, blocked or error; leon parses it per
  the spec, sends `program_status_changed` events, and answers
  `leon block call … program_status`.

## M3: layout and events - 2026-11-14

### Added

- `split`, `focus` by direction, `move`, `swap`, `zoom`, `resize` and closing panes.
- `leon events`: an event stream with a gapless `seq`, starting from a snapshot.

## M2: terminal blocks - 2026-11-14

### Added

- Real terminals in a PTY: `leon run`, `send`, `send-key`, `capture` and `wait`, and `run --wait`
  with the program's exit status. Programs see `LEON_SESSION` and `LEON_BLOCK`.

## M1: server, API and sessions - 2026-11-14

### Added

- `leon server run|start|stop|status`; any command starts the server when it isn't running.
- Sessions and windows: create, list, rename and close them; targets by ID, label, position or ID
  suffix.
- The self-describing API: `help.all`, and `leon api list|describe|call`.
