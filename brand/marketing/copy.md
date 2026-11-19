# leon copy

## Tagline

Keep your work in view.

## One line

leon is a terminal multiplexer for macOS and Linux. Run shells, builds and coding agents side by
side, and see which one needs you.

## Alternatives

- Many terminals. One place to look.
- Room for every running thing.
- Your shell, with a wider view.

## 50-word pitch

leon is a terminal multiplexer for people running many things at once. Keep shells, tests and
coding agents in split panes, tabs and sessions that keep running when the app quits. Drive them
from the native macOS app, the CLI on macOS and Linux, or Lua. See which programs are working,
blocked or done.

## 150-word description

leon gives your terminal work a place to stay. Open a session for each project, put shells in
tabs, and split a tab when you need to see two things together. A sidebar keeps your sessions
within reach.

The macOS app is native, built in Rust. Its CLI and self-describing API let scripts open panes,
send input, capture output and wait for a program to finish. Lua gives you key bindings, actions
and scripts that respond to events.

Coding agents can use those same controls. Install the built-in skill with `leon skill setup`,
then ask your agent to run the tests in a pane beside it and read the result. Programs can report
working, blocked or done, so you can see where you are needed.

Use leon for the work you already do: a shell, a build, a server and an agent, all in view on your
Mac.

## Accuracy notes

- The pictures on the landing page are the docs' own screenshots and demo (`docs/images/`), made
  smaller by `media.sh`.
- Status is program-reported, not inferred from every process automatically.
- Install with Homebrew (`brew install --cask aindong/leon/leon` for the app, `brew install aindong/leon/leon` for the CLI alone) or from a release; `scripts/install.sh` builds from a checkout. Leon.app needs macOS 11+ and is not notarized yet; the CLI runs on macOS and Linux.
- Running jobs survive quitting the app (it detaches; the server keeps them). Do not claim they survive the server stopping or a reboot: saved layouts restore with fresh shells, not resumed processes.
