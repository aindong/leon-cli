# leon copy

## Tagline

Keep your work in view.

## Alternatives

- Many terminals. One place to look.
- Room for every running thing.
- Your shell, with a wider view.

## 50-word pitch

leon is a native macOS terminal for people running many things at once. Keep shells, tests, and coding agents in split panes, tabs, and sessions. Drive the same workspace from the keyboard, CLI, or Lua. See which programs are working, blocked, or done, when they report their status to leon.

## 150-word description

leon gives your terminal work a place to stay organised. Open a session for each project, put shells in tabs, and split a tab when you need to see two things together. A sidebar keeps your sessions within reach.

The native macOS app is built in Rust with GPUI. Its CLI and self-describing API let scripts open panes, send input, capture output, and wait for a program to finish. Lua gives you key bindings, actions, and scripts that respond to events.

Coding agents can use those same controls. Install the built-in skill for Claude Code or Codex, then ask your agent to run tests in a neighbouring pane and read the result. Programs can report working, blocked, or done, so you can see where attention is needed.

Use leon for the work you already do: a shell, a build, a server, and an agent, all within view on your Mac.

## Accuracy notes

- The demo graphics illustrate a workflow; they are not app screenshots or benchmark results.
- Status is program-reported, not inferred from every process automatically.
- Install from a repository checkout with `scripts/install.sh`; no Homebrew formula or remote installer is claimed.
- Do not claim running jobs survive quitting the app. Saved layouts restore with fresh shells, not resumed processes.
