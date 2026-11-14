# Agents drive leon

A coding agent (Claude Code, Codex) running in a leon terminal can do what you do there: open a
pane beside itself, run the tests in it, wait for them, read the screen, type into a REPL, and
close what it opened. It uses the same `leon` commands you do. This page explains how to teach it,
and how an agent should behave once it knows.

## Set it up once

```sh
leon skill setup --dry-run   # show what it would write
leon skill setup             # install for every agent it finds
leon skill status            # current, outdated, missing, foreign or unreadable
```

`leon skill setup` installs two things for each agent it finds (`--agent claude|codex` picks one):

| | Claude Code | Codex |
|---|---|---|
| The skill: a `SKILL.md` stub | `~/.claude/skills/leon/` (`$CLAUDE_CONFIG_DIR`) | `~/.agents/skills/leon/` |
| A session-start hook running `leon skill hook` | `~/.claude/settings.json` | `~/.codex/hooks.json` (`$CODEX_HOME`) |

- **The skill** tells the agent to run `leon skill show`, which prints the full guide compiled into
  your `leon` binary. Upgrading leon upgrades the guide; there is nothing to reinstall.
- **The hook** runs when an agent session starts. Inside leon it tells the agent its session and
  terminal block; outside leon it prints nothing and never contacts the server.

Setup is safe to run again. It edits settings files in place, keeping your keys and their order,
backs a file up once to `<file>.leon-backup`, and refuses a file that isn't the JSON shape it
expects. It won't replace a `SKILL.md` it didn't write unless you pass `--force` (the old one is
kept as `SKILL.md.leon-backup`).

```sh
leon skill show                    # read the guide the agent reads
leon skill remove --dry-run
leon skill remove                  # removes what setup installed, and nothing else
```

## Where am I: `LEON_SESSION` and `LEON_BLOCK`

Every leon terminal has two variables:

- `LEON_SESSION`: the session's ID, e.g. `session:3f2a…`.
- `LEON_BLOCK`: this terminal's own ID, e.g. `block:91c0…`.

leon commands default to them, so an agent's `leon run` opens a tab in *its own* session, beside
the user's other work, without any flags. With neither variable set the agent isn't inside leon:
it should run `leon ls` and ask which session to use.

How an agent should read the user's words:

- "here", "beside you", "in this session": its own `LEON_SESSION`.
- "the tests pane", "session api": a name the user gives always wins over the defaults; pass it with
  `-b` or `-s`.
- If a name matches more than one thing, leon lists them and exits 3: ask, don't guess.

**One trap:** `send`, `send-key` and `capture` without `-b` act on `LEON_BLOCK`, which is the
agent's own terminal. Typing there types into the agent's own input. Always pass `-b` with the block
you mean.

## A session from the agent's side

```sh
# open a pane beside me, run the tests there, keep it after they finish
id=$(leon split --label tests --keep-open -- cargo test)
leon wait "$id"; echo "tests exited $?"
leon capture -b "$id" --trim | tail -40

# drive a REPL
py=$(leon run --label py -- python3)
leon send -b "$py" 'print(6 * 7)'
leon send-key -b "$py" enter
leon capture -b "$py" --trim | tail -3

# clean up what I created, and only that
leon block close "$id"
leon block close "$py"
```

The rules the guide gives agents:

- Label every block you create, so the user can tell yours from theirs.
- Close what you created when you're done; never close the user's blocks or your own `LEON_BLOCK`.
- Don't read or type into the user's terminals unless asked: screens hold secrets, and keys you
  send run in their shell.
- For a program that keeps running (a dev server, a REPL), don't `leon wait`; poll `leon capture`
  for the text you expect, with your own deadline.
- Use `--json` for output you parse, and `leon api list` / `leon api describe` for anything the
  commands don't cover.

## Program status: let the user see what the agent is doing

Agents and tools can report their state with OSC 7501 (`working`, `blocked`, `done`, `error`),
which leon tracks per terminal. `leon block call program_status -b "$LEON_BLOCK"` reads it back,
and each change is a `program_status_changed` event in `leon events -s SESSION`.
