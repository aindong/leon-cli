# Changelog: the detail of each feature

The detail of each feature in [CHANGELOG.md](../CHANGELOG.md) that came after its milestones,
newest first, moved here to keep that file short; each section is as it was written. The
changelog keeps one summary bullet or link per feature, pointing at its section here. The
features that shipped in 0.1.0 keep their heading here, with their detail in
[changelog-0.1.0.md](changelog-0.1.0.md).

## Since 0.1.0: the detail

### Leon.app never waits on its own server

#### Changed
- Every call the window makes to This Mac's server (a key, a paste, a split and the directory it
  opens in, a copy, a cmd-click, End All Sessions) goes through one ordered queue off the window's
  thread, as a host's calls already did: a stuck server no longer freezes the window for the 5 s
  call timeout.
- The window's copy of each machine's model changes only on its own thread, and only what a
  change touches is drawn again ([How leon uses GPUI](architecture/gpui.md), R4).
- A config reload reaches the window through the server's `keymap_changed` event, at once,
  instead of the app asking for the keymap every second.

### Overlays give the keys back

#### Changed
- The palette (and its switcher, theme list and Add Host prompt), the find bar, the overview and
  the inline rename field each take the window's focus and their own keys while open; Escape,
  their key again, a pick, or the pane they belong to going away gives the focus back to the pane.
  A key the find bar doesn't take (⌘D) still runs its binding.
- A click outside the palette closes it, as one outside the rename field already set the name.

### Diagrams and benchmark charts

#### Added
- Mermaid diagrams, which GitHub and the website both draw: the big picture and the object model
  ([architecture.md](architecture.md)), an orchestrator and its worker and the permission hold
  ([agents.md](agents.md)), links to a host ([remote-hosts.md](remote-hosts.md)), the tailnet
  listener's admission ([architecture/tailnet.md](architecture/tailnet.md)), the web page's checks
  ([web.md](web.md)), scrollback's store ([architecture/scrollback.md](architecture/scrollback.md)),
  the release run ([releases.md](releases.md)) and the update check ([updates.md](updates.md)).
  The website serves its own copy of Mermaid, pinned and checked against a recorded hash.
- Charts of leon's benchmarks, drawn from data files in `docs/perf/` by
  `scripts/perf-charts.py`, which a test runs so a chart never drifts from its data; a Performance
  section on the website opens with them ([performance.md](performance.md)). The app's own
  measurements moved to [performance-app.md](performance-app.md).

### What's new in the app

#### Added
- Leon.app's first launch of a new version shows "Updated to leon X.Y.Z." once in the update
  notice, with What's New (and Open What's New in the palette), which opens the website's page for
  that version. `updates.json` keeps the version that last ran; a first install shows nothing
  ([updates.md](updates.md#in-leonapp)).

### The website

#### Added
- <https://aindong.github.io/leon-cli/>: the landing page, the README and every doc and
  translation, with search, a language switcher and light and dark; What's new, a page for each
  release from its changelog section; and a demo page for the videos in the `media` pre-release.
- `.github/workflows/site.yml` deploys it from the latest release's tag after each release, or by
  hand; `website/build.sh` builds it, and CI builds it strictly on every change but one to
  translations, images or `brand/` alone (they start no run)
  ([releases.md](releases.md#the-website)).

### HTTPS for the browser page

#### Added
- `serve_web_cert` and `serve_web_key`: the files `tailscale cert` wrote. The page is then served
  over HTTPS on the same port and plain HTTP only redirects; leon reloads renewed files and never
  makes a certificate itself ([web.md](web.md#https)).
- `server.status`'s `tailnet.web_error` says what is wrong with the files and the command to run;
  `server.settings` gains both settings, so `api_version` goes from `1.16.0` to `1.17.0`.

### leon in the browser

#### Added
- `serve_web`: a view-only page over the tailnet ([web.md](web.md)); `server.settings` and
  `server.status` gain fields, so `api_version` goes from `1.15.0` to `1.16.0`.

### Cursor shaders

#### Added
- `leon.set("cursor_shader", PATH)` names a Shadertoy-style cursor shader file; Leon.app draws it
  over its window while the cursor's move animates, for `cursor_shader_duration` seconds (0.1 to
  5, default 1), on a layer of its own that draws nothing while idle. macOS only;
  [cursor-shaders.md](cursor-shaders.md) has the uniforms and what works.
- `leon config check` translates the file and prints the shader's own line on an error; the app
  shows the same in its notice and reloads the shader when the file is saved. The Settings
  window has it under Appearance.
- `server.settings` reports both: new result fields on a stable method, so `api_version` goes
  from `1.14.0` to `1.15.0`.

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
- leon types "You have 2 leon messages. Run leon inbox to read them." into an orchestrator's
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
  narrow one. Rows aren't copied as they scroll, only when output pauses (or once a second), so
  `cat` is as fast as before. Line numbers, styles, `capture`, restore after a restart and `leon
  attach` are unchanged ([architecture/scrollback.md](architecture/scrollback.md)).
- Stored lines aren't re-wrapped when a pane is resized: a narrower pane cuts them at its edge
  (their whole text stays in `capture`).
- A pane made taller or wider fills its new rows from history as before, whether the history is
  still alacritty's or already stored.
- Find, selection, copy and links in the app reach the stored lines too, with the same results as
  when alacritty held them all.

#### Added
- `leon.set("scrollback", N)` (0 to 100000, default 10000) and `leon config set scrollback N`: the
  lines of history each pane keeps ([lua/settings.md](lua/settings.md)). A change applies to
  running panes at once, and a lower value drops their oldest lines.
- `server.settings` reports `scrollback`: a new result field on a stable method, so `api_version`
  goes from `1.17.0` to `1.18.0`.

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
