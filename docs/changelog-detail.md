# Changelog: the detail of each feature

The detail of each feature in [CHANGELOG.md](../CHANGELOG.md) that came after its milestones,
newest first, moved here to keep that file short; each section is as it was written. The
changelog keeps one summary bullet or link per feature, pointing at its section here. The
features that shipped in 0.1.0 keep their heading here, with their detail in
[changelog-0.1.0.md](changelog-0.1.0.md).

## Since 0.1.0: the detail

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
- API (experimental): `agent.link`, `agent.list`, `agent.inbox`; `dev.leon.terminal.report_status`
  takes `kind`. `api_version` goes from `1.12.0` to `1.13.0`.

#### Changed
- A block whose program reports `blocked`/`permission` is held for a person: until a person's
  keys reach it, no program running inside a leon terminal can type into it, change its status from
  outside it, or run config actions or reloads (error `permission_pending`). The app, and
  `leon attach` in a terminal app, type into it as before.
- The state file is version 5: it keeps each worker's link and up to 1024 unread reports.
- The guide says when not to use leon, documents `-H`, what to do on exit 4, and looking before
  repeating a `send`.

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
