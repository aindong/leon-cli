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
| M11 | Scrollback and copy | Wheel and trackpad scrollback fetched from the server, selection (word, line, rectangle), copy and paste, mouse reporting for programs that ask, find in the pane. |
| M12 | Switch fast | ⌘K switches to any session, tab or block, the most recent first; the command palette runs any action. |
| M13 | Attention | Status in the app, the Activity list (⌘⇧A), the Dock badge, notifications, and `leon notify` for tools that can't print OSC 7501. |
| M14 | Agents come back | An agent's pane records the command that resumes its conversation; after a server restart the pane offers it (the `resume` setting), and its scrollback comes back. |
| M15 | Linux | The server, the CLI and `leon attach` on Linux (x86_64 and arm64), with a tarball and a `.deb`. The app on Linux is still to come. |
| M16 | Remote hosts | `-H HOST` over the user's own ssh; the app shows each host's sessions and survives a dropped link. |
| M17 | `leon attach` | A terminal client: split panes, a status bar and the prefix keys, in any terminal or over ssh. |
| M18 | A promise for integrators | A stable, versioned socket API with a changelog and a guide for writing clients. |
| M19 | Keys for terminal habits | Repeatable bindings, copy mode, hints, and a preset for prefix-key users. |
| M20 | Floating panes and drawers | Float a pane above the tiles, dock one as a drawer, and a scratch drawer (⌘J). |
| M21 | Projects and worktrees | Project and git worktree sessions as a Lua recipe. |
| M22 | Themes and settings | Light and dark themes, theme import, `leon config get/set`, and the app's Settings window. |

## Next

- The app on Linux.
- Two clients on one tab: with the app and `leon attach` on the same tab, each pane takes the size
  of the client that last changed its layout, and the other client clips or pads it.
- Lookalike link hosts: ⌘-click opens an `http(s)` link without asking even when its host uses
  non-ASCII letters that look like another's.
- `codex exec` runs aren't yet told apart from interactive Codex sessions, so one can be offered a
  resume meant for an interactive session.
