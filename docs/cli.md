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
  block, so commands act "here" without flags. Outside leon, pass `-s`/`-b`; a session's commands
  then act on its active window's focused block.
- **The server.** Any command that needs the server starts a personal one if none runs
  (`--autostart=false` or `LEON_AUTOSTART=0` turns that off). The socket lives in
  `$LEON_RUNTIME_DIR`, else `$XDG_RUNTIME_DIR/leon`, else `~/.local/state/leon/run`.

## Global flags

These work with every command.

| Flag | What it does |
|---|---|
| `--autostart[=true\|false]` | Start a personal server when none is running (default true; env `LEON_AUTOSTART`). |
| `--json` | Print JSON instead of text. |
| `-s, --session SESSION` | Target session (default `$LEON_SESSION`). |
| `-b, --block BLOCK` | Target block (default `$LEON_BLOCK`, else the active window's focused block). |
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

## Sessions

### `leon new [LABEL]`

Create a session and print its ID. Same as `leon session new`.

### `leon ls`

List sessions. Same as `leon session ls`.

### `leon kill SESSION…`

Destroy sessions and end every program in them. Same as `leon session kill`.

### `leon session`

Manage sessions.

### `leon session new [LABEL]`

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
through a shell; for pipes or `&&`, use `leon run -- sh -c '…'`.

- `--label LABEL`: name the block, so `-b LABEL` finds it.
- `--keep-open`: keep the block after the program exits, so you can still read its output.
- `--wait`: wait for the program and exit with its status.
- `-c, --cwd DIR`: working directory (default: the current directory).

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

## Layout

### `leon focus TARGET`

Focus a block, or move focus `left`, `right`, `up`, `down`, `next` or `prev` in the active window.

- `--wrap`: with nothing in that direction, go round to the far side.

### `leon move [BLOCK] DIRECTION ANCHOR`

Put a block (default: the focused one) beside `ANCHOR`, to its `right`, `left`, `above` or
`below`. Both must be in the same session; they may be in different windows.

- `--ratio PCT`: the moved block's share of the space (default 50).

### `leon swap BLOCK OTHER`

Two blocks trade places.

### `leon zoom [BLOCK]`

Show a block filling its window, or stop. Toggles by default.

- `--on`: zoom in, even if already zoomed.
- `--off`: zoom out, even if not zoomed.

### `leon resize [BLOCK]`

Set a block's share of the split it is in.

- `--ratio PCT`: the block's share, 1–99 (required).

```sh
leon focus left
leon move logs below tests --ratio 30
leon zoom -b tests --on
leon swap logs tests
leon resize -b logs --ratio 25
```

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

List the server's config actions.

### `leon keymap`

Print the server's key bindings and modes, as resolved from `init.lua`.

### `leon api`

List, describe and call the server's API methods directly.

### `leon api list`

List every method.

### `leon api describe METHOD`

Show a method's description and JSON Schemas.

### `leon api call METHOD [ARGS]`

Call a method with JSON params (inline, `@file`, or `-` for stdin). `-s` fills `session_id`.

```sh
leon api list
leon api describe session.view
leon api call server.status
```

## Config

### `leon config`

Check or reload the Lua config (`init.lua`).

### `leon config check [PATH]`

Load a config without applying it: print counts and diagnostics, exit 1 if it fails or any
`leon.*` call was skipped. Needs no server. The default path is `$LEON_CONFIG`, else
`$XDG_CONFIG_HOME/leon/init.lua`, else `~/.config/leon/init.lua`.

### `leon config reload`

Re-read the config on the server. A failed load keeps the last good one and exits 1.

```sh
leon config check
leon config check ./init.lua
leon config reload
```

## Server

### `leon server`

Run, start, stop or check the server.

### `leon server run`

Run the server in the foreground. Sessions are saved in `$LEON_STATE_DIR` (default
`~/.local/state/leon`) when it stops, and restored when it starts. A restored block runs a fresh
shell under the text it showed before, and offers its resume command if a program registered one;
nothing else that ran is run again.

- `--config PATH`: the Lua config (default as for `leon config check`).
- `--restore[=BOOL]`: restore the sessions saved when the last server stopped (default true).

### `leon server start`

Start a server in the background. Does nothing if one runs (except make it persistent).

- `--restore[=BOOL]`: restore the sessions saved when the last server stopped (default true).

### `leon server stop`

Stop the server. Every program in every session ends; the sessions are saved for the next start.

- `--discard`: don't save, and forget the sessions saved before, so the next start is empty.

### `leon server status`

Show whether a server runs; exit 4 if not.

## Agents

### `leon skill`

Teach coding agents (Claude Code, Codex) to drive leon. Needs no server. See [agents.md](agents.md).

### `leon skill show`

Print the guide for agents driving leon (a SKILL.md).

### `leon skill setup`

Install the skill and a session-start hook for each agent found. The hook tells the agent where
it is and records how to resume its session after a restart.

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
