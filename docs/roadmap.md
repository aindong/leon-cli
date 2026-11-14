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
| M10 | The app attaches | Opening the app attaches to the running server (starting one if needed); quitting it leaves every program running; "Quit and End All Sessions…" or `leon server stop` ends them; screens are pushed over the socket within the latency budget in docs/performance.md. |
| M10b | The UI pass | Tabs in one place (sidebar rows by default, the titlebar with `client.tab_style.toggle`, saved across launches); a quiet sidebar that resizes; leon's brand palette from one theme module; status once per place; pane headers with controls; inset pane tiles; one toolbar row; program-set titles; an empty state that teaches. |
