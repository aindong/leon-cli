# leon command reference

Every `leon` command and flag. `leon <command> --help` prints the same text in short; a test
(`crates/leon/tests/m7b_cli_doc.rs`) fails if this page and its parts in `docs/cli/` miss a command
or flag that `--help` shows.

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
  through your own `ssh` (see [cli/hosts.md](cli/hosts.md)).

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
| 10 | `self-update --check`: a newer release is available |
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

Over `-H`, the status bar says `reconnecting to HOST (try N)…` while a dropped link is retried,
and keys reach the pane whole or not at all. After a second with no word from the host, typed keys
wait for a quick check of the link before they go; if it fails, or the link is reconnecting, they
are dropped, and the bar says how many keys were not sent. Type them again once it is back. A
paste waits in line with the keys. One limit: a stall that starts within that second, mid-word,
can still catch the keys already on their way, and those may arrive late.

## Sessions and windows

`leon new`, `ls`, `kill`, `leon session` and `leon window` are in [cli/sessions.md](cli/sessions.md).

## Running programs

### `leon run [FLAGS] -- CMD…`

Run a command in a new window and print the new block's ID. The command runs directly, not
through a shell; for pipes or `&&`, use `leon run -- sh -c '…'`. It is found on your login shell's
PATH, even when the server was started by Leon.app from Finder.

- `--label LABEL`: name the block, so `-b LABEL` finds it.
- `--keep-open`: keep the block after the program exits, so you can still read its output.
- `--wait`: wait for the program and exit with its status.
- `--output[=true|false]`: with `--wait`, print the program's text after the ID line once it
  exits: what `leon capture --trim` would show, scrollback and screen. On by default when stdout
  is not a terminal (an agent's or a script's `$(…)`), off on a terminal. To read the text, `run`
  keeps the block open until it has been read, then closes it (unless `--keep-open`); a
  `run --wait` killed before then leaves its block open, as `--keep-open` would.
- `-c, --cwd DIR`: working directory (default: the current directory; with `-H`, the host's
  home, and `DIR` must be an absolute path there).

```sh
leon run --label tests --keep-open -- cargo test
leon run --wait -- make                       # in a script: make's output, then its status
leon run --wait --output=false -- make        # the status alone
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
- `--text TEXT`: wait until the block's screen or scrollback shows `TEXT` instead, and exit 0
  (text already there counts). Exits 1 after `--for`, or if the block closes first. For a
  program that keeps running: a prompt, a dev server's "ready" line.

```sh
id=$(leon run --keep-open -- python3)
leon wait -b "$id" --text '>>>' --for 10s    # Python's prompt: now it reads what you type
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
- `--kind permission|question|auth`: what a `blocked` program waits for (with `--state blocked`).
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
clears it); a restored block offers it by the config's `resume` setting (see [lua/settings.md](lua/settings.md)):

```sh
leon block call set_resume '{"command": ["claude", "--resume", "c7583d2c-3133-4d13-b5f0-45883f1cabcb"]}'
```

## Automation

`leon events`, `leon do`, `leon actions`, `leon keymap` and `leon api` are in [cli/automation.md](cli/automation.md).

## Config and themes

`leon config` (check, reload, get, set) and `leon theme` (list, import, show) are in
[cli/settings.md](cli/settings.md).

## Server

`leon server` (run, start, stop, status, proxy) is in [cli/server.md](cli/server.md).

## Hosts

`-H HOST` reaches the leon server on another machine through the system `ssh`. `leon hosts`
(list, add, remove, check; `--tailscale` adds a machine from your tailnet) and how `-H` works are
in [cli/hosts.md](cli/hosts.md).

## Updates

### `leon self-update`

Updates leon to the newest release; `--check` only reports (exit 10): [updates.md](updates.md).

## Agents

`leon skill` (show, setup, status, remove), `leon agent`, `leon inbox`, `leon float`, `leon toggle` and `leon dock` are in [cli/agents.md](cli/agents.md).
