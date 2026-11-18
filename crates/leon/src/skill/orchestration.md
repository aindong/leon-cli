# Orchestrate other agents with leon

You can start other coding agents (Claude Code, Codex, or any agent CLI) in leon tabs or panes,
each with a task. They work for you: leon tells you when one finishes, gets stuck, or asks
something, and you answer them. Each worker can start workers of its own; their reports go to it,
not to you.

## Start a worker

```sh
id=$(leon agent start --label tests claude "Make the failing test in crates/foo pass")
leon agent start --split right codex -f task.md      # in a pane beside you; the task from a file
git diff | leon agent start --label review claude -  # the task from stdin
leon agent start claude "the task" -- --model sonnet # agent arguments after --
```

Give workers that edit files a worktree each, so they don't step on each other or on you:
`leon agent start --worktree --branch fix-parser claude "…"` runs it in a new git worktree and
branch beside the checkout.

`leon agent start AGENT TASK` runs `AGENT [ARGS…] PROMPT` in a new tab (`--split DIR` for a pane
beside you, `-c DIR` for its directory, `--label` to name it) and prints its block ID. PROMPT is
your task followed by a paragraph from leon telling the worker how to report. The worker works for
your block (`$LEON_BLOCK`, or `-b`). Label every worker: you and the user tell them apart by it.

Write the task the way you would brief a colleague who has not seen this conversation: the goal,
the files, what done looks like, and what to leave alone.

## Wait for it

Don't poll. End your turn. When a worker reports, leon types one line into your prompt once your
turn has ended and nobody has typed into your pane for a few seconds:

    You have 2 leon messages. Run leon inbox to read them.

Reports that come while you work are gathered into that one notice. leon only types into an agent
whose hooks say its turn ended (`leon skill setup` installs them). Without them, check
`leon inbox` yourself now and then.

## Read the inbox

```sh
leon inbox            # unread reports, oldest first; marks them read
leon inbox --json     # [{block_id, label, agent, state, kind, title, body, at}]
leon agent ls         # your workers: label, state, agent, age, unread, block
```

A report's state is what the worker said:

- `done`: its turn ended; `body` is its result (or its last message).
- `blocked`, kind `question`: it asks you something; `body` is the question.
- `blocked`, kind `permission`: it waits on a permission prompt (to run a command, edit a file).
  **Only the user answers those**, in the leon app; until they have typed there, leon refuses your
  `send`, `send-key` and `agent tell` to that pane. Tell the user which worker waits and what it
  asks, if they haven't seen it. Don't try to get around the refusal.
- `error`: it reported a failure.
- `exited` or `closed`: its program ended (`body` has the exit status) or its pane was closed. It
  is no longer your worker.

Each report says how to look: `leon capture -b BLOCK --trim | tail -40` shows its screen.

## Answer it

```sh
leon agent tell tests "Use the fixture in tests/data, not a new one."
```

`leon agent tell W TEXT` waits until W's agent reports its turn ended, then types TEXT and presses
Enter (`--for 30s` to give up sooner; the default is 10 minutes). It doesn't type into a worker
that reports no state (no leon hooks, or its agent quit). `leon send -b W` and
`leon send-key -b W enter` work too, but type at once, even mid-turn.

## Clean up

A worker's pane stays open after its program exits, so you can still read it. Close each one you
started when you are done with it: `leon agent close "$id"`, which also removes its worktree and
branch. If it refuses, the worktree holds work that isn't committed and pushed: look at it, keep it
(merge or push the branch), or tell the user; use `--force` only when you are sure it can go. Never
close workers you didn't start.

## If you are the worker

You were started with a task and a paragraph about reporting. Your state (working, waiting on a
permission prompt, turn ended) reaches your orchestrator through leon's hooks. Also:

```sh
leon notify --state done --body "Fixed: the parser skipped empty lines. Tests pass."
leon notify --state blocked --kind question --body "Postgres or SQLite for the test database?"
leon notify --state error --body "Can't build: the toolchain is missing."
```

Report your result in a line or two when you finish, then end your turn. To ask, report the
question and end your turn: the answer is typed into your prompt. A body is one line, at most 2048
bytes; put more in a file and name the file.
