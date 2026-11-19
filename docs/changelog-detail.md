# Changelog: the detail of each feature

The detail of each feature in [CHANGELOG.md](../CHANGELOG.md) that came after its milestones,
newest first, moved here to keep that file short; each section is as it was written. The
changelog keeps one summary bullet or link per feature, pointing at its section here. The
features that shipped in 0.1.0 keep their heading here, with their detail in
[changelog-0.1.0.md](changelog-0.1.0.md).

## Since 0.1.0: the detail

### Closing and renaming tabs and sessions

#### Added
- In Leon.app's command palette: Close Tab (⌥⌘W), Close Session (⌥⇧⌘W), Rename Tab… and Rename
  Session…. A close asks first when a program other than a shell is in front in that tab or
  session, naming it, as quitting does; only shells close at once. A server older than the
  `foreground` field names no programs, so its tabs and sessions close without asking.
- A × on hover on each sidebar tab row and session, top style's session rows, and titlebar tabs
  (whose × used to close without asking), by the same rule.
- A right-click on a sidebar row or a titlebar tab opens a menu: Rename Tab… and Close Tab, or New
  Tab, Rename Session… and Close Session, by mouse or with the arrow keys and Enter.
- A double-click on a tab's or a session's name renames it in place: type the new name, Enter or a
  click elsewhere sets it, Escape cancels. An empty name gives the tab its program's title again.

### The link card

#### Added
- In Leon.app, resting the mouse on a link for about half a second, with no key held, shows a card
  under it: the full address with hidden characters stripped, what it opens when escapes change it
  (only what follows the host is decoded), and the host on a line of its own whenever user-info, an
  escape or a non-ASCII script could disguise it (as punycode) or the address was cut; then Copy and
  Open in Browser. A right-click on a link opens the same card at once, ready for the arrow keys and
  Enter. A program that asked for the mouse (vim, htop) gets the click and shows no card. ⌘-click
  still opens a link at once, and its prompt shows the same lines. A card the mouse opened closes
  when the mouse leaves the link and the card, on any key, and on scroll; the right-click card
  closes on Escape, on any key but the arrows and Enter, and on a click outside it.
- `leon.set("link_card", false)` turns the card off (a toggle in Settings, Terminal).
  `server.settings` reports `link_card`, a new result field on a stable method, so `api_version`
  goes from `1.13.0` to `1.14.0`.

### Agents orchestrating agents

#### Added
- `leon agent start AGENT TASK` starts an agent (`claude`, `codex`, or any program that takes a
  prompt as its last argument) in a new tab, or a pane with `--split DIR`, with the task (text,
  `--file`, or `-` for stdin) and a paragraph on how to report as its first prompt; the caller's
  block is its parent. Workers can start workers of their own. `leon agent ls` lists your workers,
  `leon agent tell W TEXT` types an answer once a worker's turn has ended, and `leon inbox` reads
  their reports ([agents.md](agents.md#orchestrate-other-agents)).
- `leon skill setup` installs status hooks beside the session-start one: a prompt sets the block
  `working`, a permission prompt `blocked`/`permission` with what it asks, Claude Code's questions
  `blocked`/`question`, the end of a turn `done` with its last message, Codex's interrupt `idle`.
  Run `leon skill setup` again to get them; every agent pane then shows its state.
- leon types "You have N leon messages. Run leon inbox to read them." into an orchestrator's
  prompt once the program in front of its pane reports its turn ended and nobody typed there for
  3 s: one notice for every report that came meanwhile, never into a shell.
- `leon notify --kind permission|question|auth`; `leon skill show orchestration`.
- `leon agent start --worktree [--branch NAME]` gives a worker a git worktree and branch of its own
  beside the checkout, and `leon agent close W [--force]` closes it and removes both, refusing while
  they hold work that isn't committed and pushed (ignored files and a lock count too).
- API (experimental): `agent.link`, `agent.list`, `agent.inbox`, with a worker's `worktree`;
  `dev.leon.terminal.report_status` takes `kind`. `api_version` goes from `1.12.0` to `1.13.0`.

#### Changed
- A block whose program reports `blocked`/`permission` is held for a person: until a person's
  keys reach it, no program running inside a leon terminal can type into it, change its status from
  outside it, or run config actions or reloads (error `permission_pending`). The app, and
  `leon attach` in a terminal app, type into it as before.
- The state file is version 5: it keeps each worker's link and worktree, and up to 1024 unread
  reports.
- The guide says when not to use leon, documents `-H`, what to do on exit 4, and looking before
  repeating a `send`.

### Scrollback that costs what its text costs

#### Changed
- When output pauses, a pane's history moves out of alacritty, which keeps every row at the pane's
  full width, into leon's own store of each row's text and runs of style: 10,000 lines of 70
  characters at 80 columns cost about 2.2 MB instead of 19.7 MB, and a wide pane no more than a
  narrow one. While output streams nothing is copied, so `cat` is as fast as before. Line numbers,
  styles, `capture`, restore after a restart and `leon attach` are unchanged
  ([architecture/scrollback.md](architecture/scrollback.md)).
- Stored lines aren't re-wrapped when a pane is resized: a narrower pane cuts them at its edge
  (their whole text stays in `capture`).
- Find, selection, copy and links in the app reach the stored lines too, with the same results as
  when alacritty held them all.

#### Added
- `leon.set("scrollback", N)` (0 to 100000, default 10000) and `leon config set scrollback N`: the
  lines of history each pane keeps ([lua/settings.md](lua/settings.md)). A change applies to
  running panes at once, and a lower value drops their oldest lines.
- `server.settings` reports `scrollback`: a new result field on a stable method, so `api_version`
  goes from `1.14.0` to `1.15.0`.

### Contributing and translations

#### Added
- CONTRIBUTING.md, a Code of Conduct, SECURITY.md, issue forms and a pull request template.
- Translations in `docs/i18n/<lang>/`, with `scripts/i18n-check.sh` and `scripts/i18n-status.sh`
  ([docs/i18n/README.md](i18n/README.md)); the README links each language.

#### Changed
- CI: translations, images and community files alone start no run; English docs alone run only the
  doc tests, on Linux.

### Popups

In 0.1.0: [its detail](changelog-0.1.0.md#popups).

### Server and Remote Hosts in Settings

In 0.1.0: [its detail](changelog-0.1.0.md#server-and-remote-hosts-in-settings).

### A Settings window with sections

In 0.1.0: [its detail](changelog-0.1.0.md#a-settings-window-with-sections).

### Serve over Tailscale

In 0.1.0: [its detail](changelog-0.1.0.md#serve-over-tailscale).

### An icon per program in Leon.app

In 0.1.0: [its detail](changelog-0.1.0.md#an-icon-per-program-in-leonapp).

### A translucent window in Leon.app

In 0.1.0: [its detail](changelog-0.1.0.md#a-translucent-window-in-leonapp).

### Themes

In 0.1.0: [its detail](changelog-0.1.0.md#themes).

### Terminals under load

In 0.1.0: [its detail](changelog-0.1.0.md#terminals-under-load).

### The overview in Leon.app

In 0.1.0: [its detail](changelog-0.1.0.md#the-overview-in-leonapp).

### Output throughput

In 0.1.0: [its detail](changelog-0.1.0.md#output-throughput).

### Updates in Leon.app

In 0.1.0: [its detail](changelog-0.1.0.md#updates-in-leonapp).

### Memory

In 0.1.0: [its detail](changelog-0.1.0.md#memory).

### Fading the panes without the focus

In 0.1.0: [its detail](changelog-0.1.0.md#fading-the-panes-without-the-focus).

### Updates

In 0.1.0: [its detail](changelog-0.1.0.md#updates).

### Releases

In 0.1.0: [its detail](changelog-0.1.0.md#releases).

### Daily-driver polish

In 0.1.0: [its detail](changelog-0.1.0.md#daily-driver-polish).

### M22b — Themes and settings in the app

In 0.1.0: [its detail](changelog-0.1.0.md#m22b--themes-and-settings-in-the-app).
