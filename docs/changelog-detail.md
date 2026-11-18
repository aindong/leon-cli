# Changelog: the detail of each feature

The detail of each feature in [CHANGELOG.md](../CHANGELOG.md) that came after its milestones,
newest first, moved here to keep that file short; each section is as it was written. The
changelog keeps one summary bullet or link per feature, pointing at its section here. The
features that shipped in 0.1.0 keep their heading here, with their detail in
[changelog-0.1.0.md](changelog-0.1.0.md).

## Since 0.1.0: the detail

### The link card

#### Added
- In Leon.app, resting the mouse on a link for about half a second, with no key held, shows a card
  under it: the full address with hidden characters stripped, what it opens when escapes change it
  (only what follows the host is decoded), and the host on a line of its own whenever user-info, an
  escape or a non-ASCII script could disguise it (as punycode) or the address was cut; then Copy and
  Open in Browser. A right-click on a link opens the same card at once, ready for the arrow keys and
  Enter. A program that asked for the mouse (vim, htop) gets the click and shows no card. ⌘-click
  still opens a link at once, and its prompt shows the same lines. The card closes when the mouse
  leaves the link and the card, on any key, and on scroll.
- `leon.set("link_card", false)` turns the card off (a toggle in Settings, Terminal).
  `server.settings` reports `link_card`, a new result field on a stable method, so `api_version`
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
