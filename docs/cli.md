# leon command reference

Every `leon` command and flag. `leon <command> --help` prints the same text in short; a test
(`crates/leon/tests/m7b_cli_doc.rs`) fails if this page misses a command or flag that `--help` shows.

## How commands find things

leon keeps **sessions**; a session holds **windows** (tabs); a window holds **blocks** (terminal
panes) in a split layout. Every object has a stable ID such as `block:91c0…`.

- **Targets.** `-s` picks a session and `-b` a block. A target can be a full ID, a label (any
  case), a 1-based position (windows count in tab order), or the last 4 or more characters of an
  ID. If a target matches more than one thing, leon lists the matches and exits 3.
- **Defaults.** Inside a leon terminal, `LEON_SESSION` and `LEON_BLOCK` name your own session and
  block, so commands act "here" without flags. Your session is the one holding `LEON_BLOCK` now
  (a pane can join another session), else `LEON_SESSION`. Outside leon, pass `-s`/`-b`; a session's commands
  then act on its active window's focused block.
- **The server.** Any command that needs the server starts a personal one if none runs
  (`--autostart=false` or `LEON_AUTOSTART=0` turns that off). The socket lives in
  `$LEON_RUNTIME_DIR`, else `$XDG_RUNTIME_DIR/leon`, else `~/.local/state/leon/run`; its path
  must fit a Unix socket (under 104 bytes on macOS, 108 on Linux), or leon says so and stops.
- **Another machine.** `-H HOST` sends any client command to the leon server on another machine,
  through your own `ssh` (see [Hosts](#hosts)).

## Global flags

These work with every command.

| Flag | What it does |
|---|---|
| `--autostart[=true\|false]` | Start a personal server when none is running (default true; env `LEON_AUTOSTART`). |
| `--json` | Print JSON instead of text. |
| `-s, --session SESSION` | Target session (default `$LEON_SESSION`). |
| `-b, --block BLOCK` | Target block (default `$LEON_BLOCK`, else the active window's focused block). |
| `-H, --host HOST` | Act on another machine's leon server, over ssh: a label from `leon hosts`, or `ssh://[USER@]HOST[:PORT]`. Also `-S, --server`. With it, `$LEON_SESSION` and `$LEON_BLOCK` are ignored (they name this machine's objects). |
| `-h, --help` | Print help. |
| `-V, --version` | Print the version. |

## Exit codes

| Code | Meaning |
|---|---|
| 0 | ok |
| 1 | failed |
| 2 | bad command line (`leon` with a typo suggests the nearest command) |
| 3 | target not found or ambiguous |
| 4 | server unreachable |
| 130 | interrupted (Ctrl-C) |

`leon wait` and `leon run --wait` exit with the program's own status instead.

## The app

### `leon app`

Open the leon app, attached to the server: the one already running (one the CLI started is
fine), or one it starts first, as `leon server start` does. The CLI and agents drive the same
sessions you see. `leon` with no command does the same. Quitting the app (⌘Q, closing the
window) detaches and every program keeps running; the app's "Quit and End All Sessions…" or
`leon server stop` ends them. With `--autostart=false` and no server it exits 4.

A build without the app (Linux for now, or macOS with `--no-default-features`) has no window:
`leon` alone runs `leon attach` (with `-s` and `-H` as given) when stdin and stdout are a
terminal, and otherwise prints the help and exits 0 (exit 2 if `-s` or `-H` was given); `leon app`
exits 1 saying so.

## In a terminal

### `leon attach [TARGET]`

Use leon inside any terminal, locally or over ssh (`ssh -t HOST leon attach`; without `-t` there is
no terminal and it exits 1). With no TARGET (and no `-s`) it opens a picker of every session, its
tabs and their blocks, with the selected block's screen previewed live: type to filter (letters in
order, as the command palette matches), ↑ and ↓ (or ctrl+p, ctrl+n) move, Enter attaches, Esc goes
back or quits. TARGET is a session, or a block, whose tab then opens with it focused. With no
session at all, Enter starts one.

Attached, it shows the session's active tab: every pane of the split layout with borders between
them, and a status bar with the session and its tabs. Keys go to the focused pane, except the
prefix key and your config's bindings (`init.lua`; a `cmd` key never arrives from a terminal).
After the prefix (`--prefix KEY`, else the config's `leon.mode("attach", { prefix = … })`, else
ctrl+b):

| Key | Does |
|---|---|
| `d` | Detach: `leon attach` exits 0 and every program keeps running |
| ←↑↓→ or `h` `j` `k` `l` | Focus the pane that way |
| `%`, `"` | Split the pane right, down |
| `z` | Zoom the pane (again to undo) |
| `c` | New tab |
| `n`, `p` | Next, previous tab |
| `s` | Back to the picker |
| the prefix again | Type the prefix key into the pane |

Panes are sized to fit this terminal whenever its own layout of them changes (attaching, a resize,
a split or zoom, a pane coming back on screen); a pane another client resized since is clipped or
padded. On exit
(detach, the server stopping, SIGHUP, SIGTERM, SIGINT, SIGQUIT) the terminal is given back as it was; a
watchdog process does it even after SIGKILL. Killed by a signal, it exits 128 + the signal.

## Sessions

### `leon new [LABEL] [--cwd DIR]`

Create a session and print its ID. Same as `leon session new`.

- `-c, --cwd DIR`: start the first terminal in this existing directory. Relative paths resolve
  against the caller's working directory. A missing path or a file exits 2 (usage error).
  Omitted uses the server's home directory. With `-H` the path is the host's: it must be
  absolute, and the host checks it.

### `leon ls`

List sessions. Same as `leon session ls`.

### `leon kill SESSION…`

Destroy sessions and end every program in them. Same as `leon session kill`.

### `leon session`

Manage sessions.

### `leon session new [LABEL] [--cwd DIR]`

Create a session and print its ID.

### `leon session ls`

List sessions: ID, label, window count.

### `leon session inspect [SESSION]`

Show a session and its windows (default: `-s`, else `$LEON_SESSION`).

### `leon session rename SESSION LABEL`

Rename a session.

### `leon session kill SESSION…`

Destroy sessions.

```sh
leon new work
leon session rename work "my project"
leon ls --json
```

## Windows (tabs)

### `leon window`

Manage the windows of a session (`-s`).

### `leon window new [LABEL]`

Add a window running a login shell and print its ID.

### `leon window ls`

List windows in tab order.

### `leon window rename WINDOW LABEL`

Rename a window. `WINDOW` is an ID, label, 1-based tab position or ID suffix.

### `leon window close WINDOW`

Close a window and end its programs. Closing a session's last window destroys the session.

## Running programs

### `leon run [FLAGS] -- CMD…`

Run a command in a new window and print the new block's ID. The command runs directly, not
through a shell; for pipes or `&&`, use `leon run -- sh -c '…'`. It is found on your login shell's
PATH, even when the server was started by Leon.app from Finder.

- `--label LABEL`: name the block, so `-b LABEL` finds it.
- `--keep-open`: keep the block after the program exits, so you can still read its output.
- `--wait`: wait for the program and exit with its status.
- `-c, --cwd DIR`: working directory (default: the current directory; with `-H`, the host's
  home, and `DIR` must be an absolute path there).

```sh
leon run --label tests --keep-open -- cargo test
leon run --wait -- make
```

### `leon split [FLAGS] [-- CMD…]`

Split a block and run `CMD` (default: a login shell) in the new pane; prints the new block's ID.
It splits `-b` (which may also name a split, to put the pane beside a whole group), else the
focused block.

- `--split right|left|above|below`: where the new pane goes (default right).
- `--ratio PCT`: the new pane's share of the space, 1–99 (default 50).
- `--label LABEL`, `--keep-open`, `-c, --cwd DIR`: as for `leon run`.
- `--focus[=true|false]`: focus the new pane (default true).

```sh
leon split
leon split --split below --ratio 30 --label logs -- tail -f /var/log/system.log
leon split --focus=false -b tests -- htop
```

### `leon send [TEXT]`

Type `TEXT` into a terminal (default: read stdin). No Enter is added; follow with
`leon send-key enter`.

### `leon send-key KEY…`

Press keys in order, written in binding syntax: `enter`, `escape`, `tab`, `space`, `backspace`,
`up`, `ctrl+c`, `alt+left`, `a`…

### `leon capture`

Print a terminal's scrollback and screen as text.

- `--trim`: drop trailing spaces and trailing blank lines.

### `leon wait [BLOCK]`

Wait for a terminal's program to exit, and exit with its status (default block: `-b`, else
`$LEON_BLOCK`).

- `--for DURATION`: give up after this long (`500ms`, `10s`, `2m`) and exit 1.

```sh
id=$(leon run --keep-open -- python3)
leon send -b "$id" 'print(6 * 7)'
leon send-key -b "$id" enter
leon capture -b "$id" --trim
leon send-key -b "$id" ctrl+d
leon wait -b "$id" --for 10s
```

### `leon notify`

Report a status for your own block (`-b`, else `$LEON_BLOCK`; with neither, exit 2), as an OSC
7501 report from its program would: it shows on the tab and pane header, in the app's Activity list,
and for blocked, error and a long run's done as a notification. For tools that can't write OSC 7501
(a hook, a job with no tty). It replaces the block's root record, with that record's lifetime.

- `--state working|blocked|done|error`: the state (default `done`).
- `--title T`, `--body B`: one line each (at most 192 and 2048 bytes). Text OSC 7501 would drop
  (controls, line breaks, invisible characters) is refused (exit 1), and nothing is recorded.

```sh
id=$(leon run --keep-open -- sleep 30)
leon notify -b "$id" --state blocked --title "Deploy" --body "approve the rollout?"
leon block call -b "$id" program_status
leon block close "$id"
```

## Layout

`leon focus`, `move`, `swap`, `zoom` and `resize` are in [cli/layout.md](cli/layout.md).

## Blocks

### `leon block`

Manage blocks (terminal panes).

### `leon block ls`

List a session's blocks.

### `leon block inspect [BLOCK]`

Show a block: its window, pid, command and exit status, and its resume command if its program
registered one (`set_resume`).

### `leon block close [BLOCK]`

Close a block and end its program.

### `leon block call METHOD [ARGS]`

Call a block method, such as `size` or `program_status`, or a full name like
`dev.leon.terminal.size`. `ARGS` is a JSON object, `@file`, or `-` for stdin.

`set_resume` registers the command that resumes this block's program after a restart (`null`
clears it); a restored block offers it by the config's `resume` setting (see [lua.md](lua.md)):

```sh
leon block call set_resume '{"command": ["claude", "--resume", "c7583d2c-3133-4d13-b5f0-45883f1cabcb"]}'
```

## Automation

### `leon events`

Stream events. By default: sessions being created and destroyed. With `-s`: that session's events,
block events too. With `--all`: everything.

- `--all`: every session's events, and the server's.

With `--json` it prints a snapshot first, then one JSON line per event, each with a `seq` that
rises by exactly one.

With `-H`, a dropped link is retried for up to 60 s and the stream resumes after the last event
printed: nothing is lost or printed twice. If the host's server restarted meanwhile, it prints a
new snapshot line and goes on from there.

### `leon do ACTION|SCRIPT|-e CODE [k=v…]`

Run a config action on the server, or a Lua script in this process. A word that looks like a path
(`./x.lua`, `x.lua`) is a script; any other word is an action. `k=v` words become arguments
(values are parsed as JSON, else kept as strings). See [lua.md](lua.md).

- `-e, --eval CODE`: run this Lua code as a script.
- `-a, --action NAME`: run this action, even if its name looks like a path.
- `--args JSON`: arguments as a JSON object (inline, `@file`, or `-` for stdin); `k=v` words
  override its keys.
- `--origin ORIGIN`: the action's `ctx.origin` (default `cli`).
- `--all`: scripts get events from every session, not just `-s`'s.
- `--for DURATION`: end a script after this long, exiting 0.
- `--fail-fast`: an error in a script's handler or timer ends it (exit 1) instead of being reported.

```sh
leon do split_here
leon do greet name=Ada
leon do -e 'print(leon.session_id)'
leon do --for 1m ./watch.lua
```

### `leon actions`

Config actions, then the app's own actions.

### `leon keymap`

Print the server's key bindings and modes, as resolved from `init.lua`.

### `leon api`

List, describe and call the server's API methods directly. Block methods (`dev.leon.terminal.*`,
called through `block.method`) are listed and described too. [api.md](api.md) is the same
reference as a page, and [integrators.md](integrators.md) explains the socket API and its
compatibility promise.

### `leon api list`

List every method with its stability: `stable` (covered by the compatibility policy),
`experimental` or `internal`. `--json` gives `name`, `stability`, `description`, and `deprecated`
when the method is on its way out.

### `leon api describe METHOD`

Show a method's stability, description, JSON Schemas and, for the most-used methods, a request
and response example.

### `leon api call METHOD [ARGS]`

Call a method with JSON params (inline, `@file`, or `-` for stdin). `-s` fills `session_id`.
A block method isn't called here: it fails, pointing at `leon block call`.

```sh
leon api list
leon api describe session.view
leon api call server.status
```

## Config and themes

`leon config` (check, reload, get, set) and `leon theme` (list, import, show) are in
[cli/settings.md](cli/settings.md).

## Server

### `leon server`

Run, start, stop or check the server.

### `leon server run`

Run the server in the foreground. Sessions are saved in `$LEON_STATE_DIR` (default
`~/.local/state/leon`) while it runs and when it stops, and restored when it starts. After a crash
they come back as they were a second or two before it, with each pane's text from up to a minute
before. A restored block runs a fresh shell under the text it showed before, and offers its resume command if a program registered one;
nothing else that ran is run again.

- `--config PATH`: the Lua config (default as for `leon config check`).
- `--restore[=BOOL]`: restore the sessions the last server saved (default true).

### `leon server start`

Start a server in the background. Does nothing if one runs (except make it persistent).

- `--restore[=BOOL]`: restore the sessions the last server saved (default true).

### `leon server stop`

Stop the server. Every program in every session ends; the sessions are saved for the next start,
with the text each terminal shows (in the private state dir, 0600).

- `--discard`: don't save, and forget the sessions saved before, so the next start is empty.

### `leon server status`

Show whether a server runs; exit 4 if not.

### `leon server proxy`

Connect stdin and stdout to this machine's server, starting a persistent one first unless
`--autostart=false`. This is what `leon -H` runs on the other machine over ssh; you don't run it
yourself.

## Hosts

`-H HOST` reaches the leon server on another machine (a devbox, a cloud VM, another Mac) through
the system `ssh`, so `~/.ssh/config`, `ProxyJump`, keys and agents all work, and leon never sees a
password. Nothing listens on a network port: leon runs `ssh -T … -- [USER@]HOST` and, on the
host, `leon server proxy` (found on `PATH`, else in `~/.local/bin`), which starts the host's
server if none runs. `LEON_SSH` names another ssh program. An endpoint is only
`ssh://[USER@]HOST[:PORT]` with plain names; anything else is refused (exit 2). Each connection is
one ssh session (`leon attach` opens three), so use keys or an agent, and `ControlMaster auto` in
`~/.ssh/config` to share one login.

```text
leon -H ssh://me@devbox ls
leon hosts add devbox ssh://me@devbox
leon -H devbox attach
```

This machine's working directory means nothing on the host: with `-H`, `run`, `split` and `new`
start in the host's home unless `--cwd` gives an absolute path there (a relative one exits 2).

`leon server run|start|proxy`, `leon app`, `leon config check`, `leon skill` and `leon hosts` are
about this machine and refuse `-H`; `leon server status` and `leon server stop` work over it.

### `leon hosts`

Name other machines for `-H`, and check that they answer. The list is
`$LEON_STATE_DIR/hosts.json` (mode 0600, in the private state dir).

### `leon hosts list`

List the hosts; `--json` prints `[{"label", "endpoints", "icon"}]`.

### `leon hosts add LABEL ENDPOINT…`

Add a host: a label (letters, digits, `.`, `_`, `-`) and its `ssh://` endpoints. `-H LABEL` uses
the first. A label is added once (exit 1 if it exists).

- `--icon NAME`: an icon name for the app to show.

### `leon hosts remove LABEL…`

Remove hosts (exit 3 if one isn't there).

### `leon hosts check [LABEL…]`

Reach each host (default: every one) as a command would, and print `ok` with its leon version,
or why not. Exit 4 if any host failed.

## Agents

### `leon skill`

Teach coding agents (Claude Code, Codex) to drive leon. Needs no server. See [agents.md](agents.md).

### `leon skill show`

Print the guide for agents driving leon (a SKILL.md).

### `leon skill setup`

Install the skill and a session-start hook for each agent found. The hook runs
`leon skill hook --agent claude|codex`, which tells the agent where it is and records how to
resume its session after a restart (see [agents.md](agents.md)).

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
the active window's focused block (the session's current directory); subsequent calls toggle it. A bottom panel already occupying that side makes creation fail.

### `leon dock left|right|bottom`

Pin `-b/--block` (default the focused block) to a side of its window. `--size F` is
a fraction strictly between 0 and 1 (default 0.3). Each side holds at most one panel,
including hidden panels; an occupied side fails without changes. `leon attach` and
the app reserve room for visible panels and draw visible floats above the tiles.
