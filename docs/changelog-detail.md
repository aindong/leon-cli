# Changelog: the detail of each feature

The detail of each feature in [CHANGELOG.md](../CHANGELOG.md) that came after its milestones,
newest first, moved here to keep that file short; each section is as it was written. The
changelog keeps one summary bullet or link per feature, pointing at its section here. The
features that shipped in 0.1.0 keep their heading here, with their detail in
[changelog-0.1.0.md](changelog-0.1.0.md).

## Since 0.1.0: the detail

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
  goes from `1.12.0` to `1.13.0`.

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
