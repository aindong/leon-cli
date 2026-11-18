# CLI: agents and floating panes

Part of the [command reference](../cli.md): teaching coding agents to work in panes (`leon skill`), and floating, toggled and docked panes.

## Agents

### `leon skill`

Teach coding agents (Claude Code, Codex) to drive leon. Needs no server. See [agents.md](../agents.md).

### `leon skill show`

Print the guide for agents driving leon (a SKILL.md).

### `leon skill setup`

Install the skill and a session-start hook for each agent found: Claude Code or Codex when its
config dir (`~/.claude`, `~/.codex`) exists or its program (`claude`, `codex`) is on PATH. It
prints why it picked each agent and every file it wrote; with no agent found it installs only the
shared `~/.agents/skills/leon/SKILL.md` and says to pass `--agent`. The hook runs
`leon skill hook --agent claude|codex`, which tells the agent where it is and records how to
resume its session after a restart (see [agents.md](../agents.md)).

- `--agent claude|codex`: only this agent.
- `--dry-run`: print what would change; write nothing.
- `--force`: replace a SKILL.md leon didn't install (the old one is kept as `SKILL.md.leon-backup`).

### `leon skill status`

Show each agent's install: current, outdated, missing, foreign or unreadable.

- `--agent claude|codex`: only this agent.

### `leon skill remove`

Remove what setup installed, and nothing else.

- `--agent claude|codex`: only this agent.
- `--dry-run`: print what would change; write nothing.

```sh
leon skill setup --dry-run
leon skill setup
leon skill status
```

### `leon float [--new [-- CMD]]`

Float the block selected by `-b/--block` (default the focused block). `--new` opens a
new terminal in the active window of `-s/--session`, running CMD or a login shell.
`--label L` assigns a label; `--rect x,y,w,h` sets fractions of the whole window
(default `0.1,0.1,0.8,0.8`), which must fit inside it. A float is shown, focused and
raised. `--unfloat` returns a float or drawer to the tiles; its terminal keeps running.
`--new` prints the new block ID; `--json` prints the API result.

### `leon toggle LABEL`

Show/hide a float or drawer by its exact, case-insensitive label within `-s/--session`.
Labels must be unique there. Hidden terminals keep running.
`--create` opens a scratch login shell docked to the bottom on first use, in the directory of
the active window's focused block (the session's current directory); subsequent calls toggle it. A bottom panel already occupying that side makes creation fail (not with `--float`).
With `--create`: `--float` makes a popup (a float at `--rect`, default `0.1,0.1,0.8,0.8`)
instead; `-- CMD` runs CMD instead of a shell. It closes when its program exits, giving the
focus back, unless `--keep-open`. See [lua/keys.md](../lua/keys.md#a-popup-on-a-key).

### `leon dock left|right|bottom`

Pin `-b/--block` (default the focused block) to a side of its window. `--size F` is
a fraction strictly between 0 and 1 (default 0.3). Each side holds at most one panel,
including hidden panels; an occupied side fails without changes. `leon attach` and
the app reserve room for visible panels and draw visible floats above the tiles.
