# Roadmap

Each milestone is one run of the loop (plan, build test-first, verify, review), committed when green.

| # | Milestone | Done when |
|---|---|---|
| M1 | Server, API and sessions | `leon server run/start/stop/status` work, the CLI autostarts the server, sessions and windows can be created, listed, renamed and killed, targets resolve by ID, label, position and ID suffix, and `leon api list/describe/call` work from `help.all`. |
| M2 | Terminal blocks | A window holds real PTY terminals: `leon run`, `send`, `send-key`, `capture`, `wait`, `run --wait` with the child's exit status, `LEON_SESSION`/`LEON_BLOCK` in the child env. |
| M3 | Layout and events | `split`, `focus` (by direction), `move`, `swap`, `zoom`, `resize`, closing panes; `leon events` streams with gapless `seq`; `pwd`/`title`/`child_exited` events. |
| M4 | Program status | OSC 7501 parsed per spec, `program_status_changed` events, `leon block call ... program_status`. |
| M5 | Lua | `init.lua` (bind, sequences, modes, actions), `config check/reload`, `leon do` scripts with `leon.on`, `wait`, timers and `exec`. |
| M6 | The app | GPUI window with sidebar sessions, tabs, split panes rendering live terminals, keyboard input, key bindings from Lua, command palette, status indicators from OSC 7501. |
| M7 | Agents and shipping | `leon skill setup/status/remove/show` for Claude Code and Codex, docs, a `.app` bundle and an install script. |
| M8 | Persistence and speed | Save/restore sessions across server restarts; measured input latency and throughput, with a benchmark in the repo. |
