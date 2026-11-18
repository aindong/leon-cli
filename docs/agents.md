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

`leon skill setup` installs two things for each agent it finds: one whose config dir exists, or
whose program (`claude`, `codex`) is on your PATH, even if you have never run it. It prints, for
each agent, why it was picked and each file it wrote. With neither found it writes only the shared
`~/.agents/skills/leon/` (other agents read it too) and says so. `--agent claude|codex` picks one:

| | Claude Code | Codex |
|---|---|---|
| The skill: a `SKILL.md` stub | `~/.claude/skills/leon/` (`$CLAUDE_CONFIG_DIR`) | `~/.agents/skills/leon/` |
| Hooks running `leon skill hook --agent …`: session start, and status ([below](#orchestrate-other-agents)) | `~/.claude/settings.json` | `~/.codex/hooks.json` (`$CODEX_HOME`) |

- **The skill** tells the agent to run `leon skill show`, which prints the full guide compiled into
  your `leon` binary. Upgrading leon upgrades the guide; there is nothing to reinstall.
- **The status hooks** set the agent's block to working, waiting on a permission prompt or a
  question, or done, so every agent pane shows its state. They print nothing and do nothing outside
  leon.
- **The session-start hook** runs when an agent session starts. Inside leon it tells the agent its session and
  terminal block, and records how to resume that session (below); outside leon it prints nothing
  and never contacts the server. A hook from an older leon is `outdated`: run `leon skill setup`.

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
the user's other work, without any flags. Its own session is the one that holds `LEON_BLOCK` now:
a pane moved into another session (⌘↩ in the app's switcher) keeps the `LEON_SESSION` it started
with, since a running program's environment can't change, so leon looks the block up and uses
`LEON_SESSION` only when `LEON_BLOCK` isn't set or is gone. With neither variable set the agent isn't inside leon:
it should run `leon ls` and ask which session to use.

How an agent should read the user's words:

- "here", "beside you", "in this session": its own session, the one holding `LEON_BLOCK`.
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
py=$(leon run --label py -- python3 -q)       # -q: no banner to match
leon wait -b "$py" --text '>>>' --for 10s     # its prompt, before typing
leon send -b "$py" 'print(6 * 7)'
leon send-key -b "$py" enter
leon wait -b "$py" --text 42 --for 10s
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
- For a program that keeps running (a dev server, a REPL), don't wait for it to exit: wait for the
  text you expect, `leon wait -b ID --text TEXT --for 30s`.
- `leon run --wait -- CMD` from an agent prints the program's text and exits with its status, in
  one call.
- Use `--json` for output you parse, and `leon api list` / `leon api describe` for anything the
  commands don't cover.

## Agents come back after a restart

When the server stops (`leon server stop`, logging out, a reboot), each block's text is saved, and
a restored block shows it above a dim `— restored from <time> —` line and a fresh shell. Programs
don't survive, and a saved command is never run again. What does come back is a command a program
registered for its own block with `set_resume`. The hook does that for you: `claude --resume <id>`
for Claude Code, `codex resume <id>` for Codex, from the session id the agent gives its hook. It
records only for an agent in the foreground of its terminal, so a nested `claude -p` run by
another agent's tool doesn't replace the interactive session's command, and never for Claude
Code's print mode (`-p`, `--print`): a one-shot run has nothing to resume.

Any other program can register its own:

```sh
leon block call set_resume -b "$LEON_BLOCK" '{"command": ["my-agent", "--resume", "42"]}'
leon block inspect "$LEON_BLOCK"        # shows "resume: my-agent --resume 42"
leon block call set_resume -b "$LEON_BLOCK" '{"command": null}'
```

On restore the block follows the `resume` setting in `init.lua` (`leon.set("resume", "ask")`):
`"ask"` (the default) types the command at the new prompt and waits for Enter (Ctrl-C skips it),
`"always"` runs it, `"never"` ignores it. A command is offered at one restart: once typed it isn't saved again, so a
skipped or stale one doesn't come back each time. A resumed agent's hook registers it again.

## Orchestrate other agents

An agent can start other agents in leon tabs or panes and hear back from them. The one that starts
them is the orchestrator; the ones it starts are its workers. leon carries what they say:

```sh
id=$(leon agent start --label tests claude "Make the failing test in crates/foo pass")
leon agent ls                        # the orchestrator's workers: state, agent, age, unread
leon inbox                           # their reports, oldest first; marks them read
leon agent tell tests "Use the fixture in tests/data."   # once its turn ended
leon block close "$id"
```

- **Start.** `leon agent start AGENT TASK` runs `AGENT [ARGS…] PROMPT` in a new tab (`--split DIR`
  for a pane) with the task and a paragraph on how to report as its first prompt, and makes it a
  worker of the caller's block (`$LEON_BLOCK`, or `-b`). `claude "…"` and `codex "…"` start an
  interactive session with that prompt; any agent CLI that takes a prompt as its last argument
  works. A worker's pane stays open after its program exits. Workers can start workers of their
  own: reports go to the direct parent only.
- **Reports.** When a worker's status changes to `done`, `blocked` or `error` (by its hooks,
  `leon notify`, or an OSC 7501 report), the report goes to its orchestrator's inbox with the
  worker's label, block, agent, kind and message; `working` and `idle` don't. A report with no
  message right after an unread one of the same state and kind from the same worker adds nothing.
  When a worker's program exits or its block closes, an `exited` or `closed` report says so, and
  it leaves the team. An inbox keeps the newest 256 unread reports.
- **The notice.** leon types `You have N leon messages. Run leon inbox to read them.` into the
  orchestrator's prompt and presses Enter, once: when the program in front of its pane is the one
  whose hooks last said its turn ended (`done`, or Codex's interrupt), at least a second ago, and
  nothing was typed into its pane for 3 seconds. Reports that come while it works make one notice.
  A pane whose program never reported a state (a shell), or whose agent quit back to its shell,
  is never typed into; the notice holds no shell syntax in case.
- **Lifetime.** When the orchestrator's block closes, its inbox goes and its workers keep running,
  unlinked. When the server restarts, links and unread reports come back with the sessions (the
  agents themselves start again only through `resume`).

### The status hooks

`leon skill setup` installs these beside the session-start hook. Each runs
`leon skill hook --agent claude|codex --on EVENT`, which sets the block's root status record. They
act only inside leon, only for the agent in its terminal's foreground (not a `claude -p` another
agent runs), never fail the agent, and give up within about 2 seconds.

| Event (Claude Code 2.1, Codex 0.164) | Status |
|---|---|
| `UserPromptSubmit` | working |
| `PermissionRequest`: `tool_name`, `tool_input` | blocked, kind `permission`: "Permission: Bash", the command |
| Claude Code `PreToolUse` on `AskUserQuestion`: the first question | blocked, kind `question` |
| `PostToolUse`, Claude Code `PostToolUseFailure` | working |
| `Stop`: `last_assistant_message` | done, with that message on one line (at most 2048 bytes) |
| Codex `Interrupt` | idle |

Claude Code runs no hook when you interrupt it (Esc), so its block stays `working` until your
next prompt. Codex has no question tool; it asks in its last message, which arrives as `done`. A
subagent's tool events don't change the block's state (the main agent may still wait on a
prompt), and a turn whose agent already reported its result with `leon notify --state done --body`
keeps that result: its Stop hook adds nothing.

### Only the user answers permission prompts

When an agent's block reports `blocked` with kind `permission`, leon holds it for a person: until a
person's keys reach that pane, no program running inside a leon terminal can type into it
(`leon send`, `send-key`, `agent tell`, `api call` of `block.event`, `dev.leon.terminal.write` or
`client.key`, a `leon attach` started in a pane, a socket client of its own), change its status
from outside it, or run a config action or reload (`leon do ACTION`, `client.action`,
`server.config_reload`). Each gets error `permission_pending`. Only the block's own program (its
hooks) may change its status meanwhile, and that doesn't end the hold: a person typing does. So an
agent can't approve its own or another agent's prompt, even by writing a report to the terminal
or starting a job that outlives it.

A person answers where they are: the app's keys reach the pane (the Activity list and the
notification take them there), and so do `leon attach` in a terminal app and `leon send` from a
shell outside leon. leon tells where a client runs from its process: the peer of the socket, its
parents up to pid 1, and its session, which a job keeps after its parent exits. A client it can't
place counts as inside.

This keeps agents from answering. It is not a sandbox:

- a program running as you outside leon counts as the person, and so does a request from another
  machine (`-H`, or a client reaching this server over Tailscale, even from a pane here);
- a program that moves itself to a new session (`setsid`) and drops its terminal gets past it;
- a dialog with no hook, such as Claude Code's question whether to trust a new folder, isn't
  held: start workers in folders the agent already trusts;
- `agent.link` and `agent.inbox` don't check who asks, as `leon send` doesn't for other blocks.

## Program status: let the user see what the agent is doing

Agents and tools can report their state with OSC 7501 (`working`, `blocked`, `done`, `error`),
which leon tracks per terminal. `leon block call program_status -b "$LEON_BLOCK"` reads it back,
and each change is a `program_status_changed` event in `leon events -s SESSION`.

### The OSC 7501 format

A report is one escape sequence written to the terminal: `ESC ] 7501 ;`, then `key=value` pairs
joined by `:`, then `BEL` (or `ESC \`). A key is lower-case letters; a value uses only letters,
digits and `_ . , + / = -`. An `id` is up to 8 segments joined by `/`, each 1 to 32 of letters,
digits and `_ . + -`. Sending `ESC ] 7501 ; ? BEL` asks what the terminal supports; leon answers
`ESC ] 7501 ; ? : states=idle,working,done,blocked,error : kinds=permission,question,auth ESC \`
(without the spaces).

For example, `printf '\033]7501;state=working:progress=40\007'` says the program is 40% through.

- **Keys.** `state` (required): `idle` (nothing to report), `working`, `done`, `blocked`, `error` or `clear`. `id`
  names a record (none is the root record; `/` makes a child). `kind` (with `blocked` only):
  `permission`, `question` or `auth`. `progress`: 0 to 100, with `working` or `blocked` (none means
  unknown). `app`: `[A-Za-z0-9_.+-]{1,32}`, taken from the nearest ancestor that sets one. `title` and
  `msg`: base64 of one line of UTF-8 with no control characters.
- **Rules.** Whitespace is trimmed, a malformed pair is skipped, an unknown key is ignored and the
  last duplicate wins. An unknown `state` or a bad `id` drops the whole report (a bad `id` never
  falls back to the root). Each report replaces its record whole; `clear` removes the record and
  its children (with no `id`, every record).
- **Lifetime.** `working` and `blocked` go when the process exits or a new prompt starts (OSC 133
  `A`). `done` and `error` stay until you look at the pane. A full reset (`RIS`) wipes the records;
  a soft reset and switching to the alternate screen don't.
- **Limits.** Past any of these the report is dropped whole: the sequence 4096 bytes, a key 16,
  `msg` 2732 encoded (2048 decoded), `title` 256 (192), `app` 32, `id` 128 in all with 32 per
  segment and 8 deep. A terminal keeps 256 records and drops the least recently used.
