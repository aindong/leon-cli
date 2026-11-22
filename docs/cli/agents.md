# CLI: agents and floating panes

Part of the [command reference](../cli.md): teaching coding agents to work in panes (`leon skill`), agents starting agents (`leon agent`, `leon inbox`), and floating, toggled and docked panes.

## Agents

### `leon skill`

Teach coding agents (Claude Code, Codex) to drive leon. Needs no server. See [agents.md](../agents.md).

### `leon skill show [orchestration]`

Print the guide for agents driving leon (a SKILL.md); `orchestration` prints its part on starting
other agents.

### `leon skill setup`

Install the skill, a session-start hook and the status hooks for each agent found: Claude Code or Codex when its
config dir (`~/.claude`, `~/.codex`) exists or its program (`claude`, `codex`) is on PATH. It
prints why it picked each agent and every file it wrote; with no agent found it installs only the
shared `~/.agents/skills/leon/SKILL.md` and says to pass `--agent`. The hook runs
`leon skill hook --agent claude|codex`, which tells the agent where it is and records how to
resume its session after a restart; the status hooks run `leon skill hook --agent A --on EVENT`,
which set the agent's block's state (see [agents.md](../agents.md#the-status-hooks)).

- `--agent claude|codex`: only this agent.
- `--dry-run`: print what would change; write nothing.
- `--force`: replace a SKILL.md leon didn't install (the old one is kept as `SKILL.md.leon-backup`, or as `SKILL.md.leon-backup.2`, `.3` and so on when earlier backups exist).

### `leon skill status`

Show each agent's install: current, outdated, missing, foreign or unreadable. With `--json`,
each agent's entry also has `found`: whether the agent is on this machine (its config dir exists
or its program is on PATH, as setup decides).

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

### `leon agent`

Start agents that work for you, list them, and answer them. See
[agents.md](../agents.md#orchestrate-other-agents).

### `leon agent start AGENT [TASK] [-- ARGS]`

Start AGENT (`claude`, `codex`, or any program that takes a prompt as its last argument) with TASK
as its first prompt, followed by a paragraph from leon on how to report, in a new tab of your
session. It works for your block (`-b`, else `$LEON_BLOCK`; with neither, exit 2): its reports go to
your `leon inbox`. Prints its block ID. Its pane stays open after it exits. ARGS go to the agent
before the prompt.

- `TASK`: the task; `-` reads it from stdin. A task is at most 64 KiB.
- `-f, --file FILE`: read the task from FILE.
- `--split right|left|above|below`: open it in a pane beside you instead.
- `--label NAME`: label its block.
- `-c, --cwd DIR`: its working directory (default: the current directory).
- `--worktree`: give it a git worktree and branch of its own, made at the checkout's HEAD in
  `<checkout>-worktrees/<branch>` beside the checkout of `-c DIR` (else the current directory), and
  run it there. Outside a git checkout, exit 1. Not with `-H`.
- `--branch NAME`: the worktree's new branch (default `agent/LABEL`, else `agent/AGENT-TIME-PID`).

### `leon agent ls`

List the agents working for you (`-b`, else `$LEON_BLOCK`): label, the state each last reported,
agent, age, unread reports and block. `--json` also gives your own parent and state.

### `leon agent close BLOCK`

Close a worker's block and remove the worktree and branch `--worktree` made for it. It first checks
what that would lose: changes not committed (untracked and ignored files too), commits no other
branch or remote holds, and a lock on the worktree. With any, it exits 1 listing them and closes
nothing; after closing it checks again, and if the agent wrote something as it ended, the worktree
stays and leon prints how to remove it. It only removes a worktree that git lists where leon puts
its own, on the branch leon made (a renamed branch is left alone). A worker without a worktree just
closes. A worktree is never removed any other way: closing the block otherwise leaves it. Not with
`-H`: run it on the machine the worker's server runs on.

- `--force`: remove them anyway; that work is lost.

### `leon agent tell BLOCK TEXT`

Type TEXT and press Enter in a worker once its agent reports its turn ended; a worker that reports
no state (no leon hooks, or its agent quit) isn't typed into. Refused while it waits on a
permission prompt, which only the user answers.

- `--for DUR`: give up after DUR (e.g. `30s`, `10m`) and exit 1 (default 10m).

### `leon inbox`

Print your workers' unread reports, oldest first, each with how to look at the worker's screen,
and mark them read.

- `--peek`: list them without marking them read.

```sh
id=$(leon agent start --label tests claude "Make the failing test pass")
leon agent ls
leon inbox --peek
leon agent tell tests "Use the fixture in tests/data." --for 30s
leon block close "$id"
w=$(leon agent start --worktree --branch fix-parser --label parser claude "Fix the parser")
leon agent close "$w"
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
