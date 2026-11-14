---
name: leon
description: Drive the leon terminal multiplexer. Open terminals beside you, run commands and wait for them, read other terminals' screens and type into them. Use when LEON_SESSION is set (you run inside leon) or the user mentions leon sessions, tabs, panes or blocks.
---

# Driving leon

leon is a terminal multiplexer. A server holds **sessions**; a session holds **windows** (tabs); a
window holds **blocks** (terminal panes). The `leon` command talks to the server, so you can open
terminals, run programs in them, read their screens and type into them, the way a person would.

## Where you are

Inside a leon terminal two variables say where you are:

- `LEON_SESSION`: your session's ID, e.g. `session:3f2a…`.
- `LEON_BLOCK`: your own terminal's ID, e.g. `block:91c0…`.

Commands act on your own session by default. With neither variable set you are not inside leon:
run `leon ls` and ask the user which session to use.

Targets: `-s SESSION`, `-b BLOCK`. A target is a full ID, a label (any case), a 1-based position,
or the last 4+ characters of an ID. A name the user gives always wins over your defaults. "Here",
"beside you" and "in this session" mean your own `LEON_SESSION`. If a target is ambiguous, leon lists
the matches and exits `3`; ask the user rather than guess.

**Careful:** `send`, `send-key` and `capture` with no `-b` act on `LEON_BLOCK`, which is you (or,
with `-s`, on that session's active window). Typing into your own terminal types into your own
input. Always pass `-b` with the block you mean.

## Run a command in a new terminal

```sh
id=$(leon run --label tests --keep-open -- cargo test)
```

`leon run` opens a new window (tab) in your session, starts the command in it and prints the new
block's ID. The command runs directly, not through a shell: for pipes, `&&` or globs, use
`leon run -- sh -c 'make && make install'`.

- `--label NAME`: name the block so you and the user can find it (`-b NAME`). Label what you create.
- `--keep-open`: keep the terminal after the program exits, so you can still read its output.
  Without it the block closes when the program ends.
- `--wait`: wait for the program and exit with its status.
- `-c DIR` / `--cwd DIR`: working directory (default: yours).

To put a terminal beside you instead of in a new tab, use `leon split` (same flags plus
`--split right|left|above|below`) when your leon has it: check `leon split --help`. If it doesn't,
use `leon run`.

## Type into a terminal

```sh
leon send -b "$id" 'print(6 * 7)'      # types the text; no Enter is added
leon send-key -b "$id" enter            # presses keys
printf 'line 1\nline 2\n' | leon send -b "$id"   # text from stdin
```

`leon send-key` takes keys in binding syntax, pressed in order: `enter`, `escape`, `tab`, `space`,
`backspace`, `delete`, `up`, `down`, `left`, `right`, `home`, `end`, `pageup`, `pagedown`, `f1`–`f12`,
a character (`a`, `+`), and modifiers joined with `+`: `ctrl+c`, `ctrl+d`, `alt+left`, `shift+tab`.

## Read a screen

```sh
leon capture -b "$id" --trim
```

prints the terminal's scrollback and screen as text. `--trim` drops trailing spaces and blank lines.
Pipe it through `tail` for the latest output.

## Wait

```sh
leon wait "$id"              # until the program exits; exits with its status
leon wait "$id" --for 2m     # give up after 2 minutes and exit 1
```

`leon wait` with the full block ID works even after the block has closed. For a program that keeps
running (a dev server, a REPL), don't wait on it: poll `leon capture` for the text you expect, with
your own deadline.

## Look around

```sh
leon ls                      # sessions
leon window ls               # windows (tabs) in your session
leon block ls                # blocks in your session: label, pid, command, exit status
leon block inspect "$id"     # one block
```

Most commands take `--json` for machine-readable output.

## Clean up what you create

- Label every block you create, so the user can tell yours from theirs.
- Close each one when you are done with it: `leon block close "$id"` ends its program and closes
  the pane. `leon window close W` closes a whole tab.
- Never close a block or window you did not create unless the user asks, and never close your own
  `LEON_BLOCK`.
- Don't type into or read the user's own terminals unless they ask: their screens can hold secrets,
  and keys you send run in their shell.

## The API

Every command is a call to the server's API, and the API describes itself:

```sh
leon api list                          # every method, one line each
leon api describe session.view         # its description and JSON Schemas
leon api call session.view '{"session_id":"…"}'
leon -s "$LEON_SESSION" api call session.list_blocks   # -s fills session_id
```

Params are inline JSON, `@file`, or `-` for stdin. Use the API for what the commands don't cover.

## Exit codes

- `0`: success.
- `1`: failed (also `wait --for` timing out).
- `2`: bad command line.
- `3`: target not found or ambiguous (`leon api call` exits `1` for every failed call instead).
- `4`: server unreachable.
- `130`: interrupted.

`leon wait` and `leon run --wait` exit with the program's own status instead.

## Recipes

Run the tests in a new terminal and read the result:

```sh
id=$(leon run --label tests --keep-open -- cargo test)
leon wait "$id"; echo "exit $?"
leon capture -b "$id" --trim | tail -40
leon block close "$id"
```

Drive an interactive program:

```sh
id=$(leon run --label py -- python3)
leon send -b "$id" 'print(6 * 7)'; leon send-key -b "$id" enter
leon capture -b "$id" --trim | tail -3     # repeat until the answer shows up
leon send-key -b "$id" ctrl+d              # python exits; the block closes
```
