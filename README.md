# leon

**A fast, scriptable terminal multiplexer for macOS.** Sessions, tabs and split panes live in a
server that outlives any window. You drive them from a native app, from the `leon` command, from
Lua, or let your coding agent drive them for you. It's built in Rust, with GPUI for the app.

- **Persistent.** Close the app's window and nothing is lost until you stop the server; every
  session, tab and pane has a stable ID you can script against.
- **Scriptable from the shell.** `leon run`, `send`, `capture` and `wait` open terminals, type into
  them, read their screens and wait for programs, with fixed exit codes and `--json` everywhere.
- **Agent-ready.** One command teaches Claude Code and Codex to open panes beside themselves, run
  your tests there and read the results.
- **Lua inside.** Key bindings, modes, your own actions, and scripts that react to events.

<!-- screenshot: the app with a sidebar of sessions, tabs, and a split pane (to be added as docs/screenshot.png) -->
> _Screenshot placeholder: the leon app with sessions in the sidebar and a split window._

## Install

You need macOS 11 or later and a Rust toolchain. From a checkout of this repository:

```sh
scripts/install.sh
```

That builds `Leon.app`, copies it to `~/Applications`, and links the `leon` command into
`~/.local/bin` (use `--prefix DIR` for `DIR/bin`). It never uses root, prints each thing it does,
is safe to run again, and refuses to replace anything at those paths that it didn't install.
`scripts/install.sh --uninstall` removes both. Open **Leon** from Finder, Spotlight or the Dock, or
type `leon`.

The app runs the server itself. If a server you started from the shell already holds sessions,
the app says so and doesn't open: run `leon server stop` (it ends those sessions' programs), then
open the app again.

To build only the app: `scripts/bundle.sh` writes `target/release/Leon.app`.

## The first five commands

From any terminal (the server starts by itself):

```sh
leon new demo                                    # 1. a session called "demo"
leon -s demo run --label py -- python3           # 2. run a program in a new tab
leon -s demo send -b py 'print(6 * 7)'           # 3. type into it (no Enter yet)
leon -s demo send-key -b py enter                # 4. press Enter
leon -s demo capture -b py --trim                # 5. read its screen: 42
```

Targets (`-s` session, `-b` block) are an ID, a label, a position or the end of an ID. Inside a
leon terminal you can drop `-s`: commands act on your own session. `leon ls` lists sessions;
`leon --help` lists everything, with the exit codes at the bottom.

## Split it

```sh
leon -s demo split -b py --split below --ratio 30 -- top     # a pane under py, 30% high
leon -s demo focus up
leon -s demo zoom                                            # fill the window; again to undo
```

In the app: ⌘D splits right, ⌘⇧D splits down, ⌘⌥ and an arrow moves between panes, and dragging a
divider resizes.

## Let an agent drive it

```sh
leon skill setup
```

installs a skill and a session-start hook for Claude Code and Codex. Then, working in a leon
terminal, ask "run the tests beside you", and the agent does what you would:

```sh
id=$(leon split --label tests --keep-open -- cargo test)
leon wait "$id"
leon capture -b "$id" --trim | tail -20
leon block close "$id"
```

It knows where it is from `LEON_SESSION` and `LEON_BLOCK`, labels what it opens, and closes only
what it created. See [docs/agents.md](docs/agents.md).

## App shortcuts

| Keys | Action |
|---|---|
| ⌘N | New session |
| ⌘T | New tab |
| ⌘D | Split right |
| ⌘⇧D | Split down |
| ⌘⌥←, ⌘⌥→, ⌘⌥↑, ⌘⌥↓ | Focus the pane that way |
| ⌘⇧↩ | Zoom the pane (again to undo) |
| ⌘W | Close the pane |
| ⌘⇧], ⌘⇧[ | Next, previous tab |
| ⌘1 … ⌘9 | Go to tab 1–9 |
| ⌘V | Paste |
| ⌘B | Show or hide the sidebar |
| ⌘⇧P | Command palette: type to find an action, ↩ runs it, esc closes |
| ⌘=, ⌘⇧=, ⌘-, ⌘0 | Bigger, bigger, smaller, default font size (every pane) |
| ⌘Q | Quit (ends every program) |

These are defaults: a `leon.bind` in your `init.lua` replaces one key by key, and `leon.unbind`
removes one (⌘Q stays). Key sequences such as `ctrl+b>c` show a hint of what can follow.
A pane whose program reports its status (OSC 7501, as agents do) shows a spinner, ⚠ / ? / ⚿ when
it is blocked, ✓ when done or ✗ on an error, on its tab and its session in the sidebar; done and
error clear when you go to the pane.

## Script it in Lua

`~/.config/leon/init.lua` defines key bindings, modes and actions; `leon do` runs scripts.

```lua
leon.action{
  name = "name_session",
  title = "Name this session",
  args = { label = "string" },
  run = function(ctx, args) return leon.session.set_label{ label = args.label } end,
}
```

```sh
leon config check
leon config reload
leon do name_session label=api
leon do -e 'print(#leon.session.view().windows .. " tabs")'
```

See [docs/lua.md](docs/lua.md).

## Docs

- [docs/cli.md](docs/cli.md): every command and flag.
- [docs/lua.md](docs/lua.md): the config and scripts, with examples that a test runs.
- [docs/agents.md](docs/agents.md): agents driving leon.
- [docs/architecture.md](docs/architecture.md): how it's built. [docs/spec.md](docs/spec.md): the
  interface it matches. [docs/roadmap.md](docs/roadmap.md): what's next.

## Develop

```sh
cargo build
cargo test
cargo clippy --all-targets -- -D warnings
```

GPUI is built with its `runtime_shaders` feature, so you don't need Xcode's Metal toolchain.
