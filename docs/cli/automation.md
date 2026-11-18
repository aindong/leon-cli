# CLI: automation

Part of the [command reference](../cli.md): streaming events, running actions and scripts, the keymap, and calling the socket API.

## Automation

### `leon events`

Stream events. By default: sessions being created and destroyed. With `-s`: that session's events,
block events too. With `--all`: everything.

- `--all`: every session's events, and the server's.

With `--json` it prints a snapshot first, then one JSON line per event, each with a `seq` that
rises by exactly one.

With `-H`, a dropped link is retried for up to 60 s and the stream resumes after the last event
printed: nothing is lost or printed twice. If the host's server restarted meanwhile, it prints a
new snapshot line and goes on from there.

### `leon do ACTION|SCRIPT|-e CODE [k=v…]`

Run a config action on the server, or a Lua script in this process. A word that looks like a path
(`./x.lua`, `x.lua`) is a script; any other word is an action. `k=v` words become arguments
(values are parsed as JSON, else kept as strings). See [lua.md](../lua.md).

- `-e, --eval CODE`: run this Lua code as a script.
- `-a, --action NAME`: run this action, even if its name looks like a path.
- `--args JSON`: arguments as a JSON object (inline, `@file`, or `-` for stdin); `k=v` words
  override its keys.
- `--origin ORIGIN`: the action's `ctx.origin` (default `cli`).
- `--all`: scripts get events from every session, not just `-s`'s.
- `--for DURATION`: end a script after this long, exiting 0.
- `--fail-fast`: an error in a script's handler or timer ends it (exit 1) instead of being reported.

```sh
leon do split_here
leon do greet name=Ada
leon do -e 'print(leon.session_id)'
leon do --for 1m ./watch.lua
```

### `leon actions`

Config actions, then the app's own actions.

### `leon keymap`

Print the server's key bindings and modes, as resolved from `init.lua`.

### `leon api`

List, describe and call the server's API methods directly. Block methods (`dev.leon.terminal.*`,
called through `block.method`) are listed and described too. [api.md](../api.md) is the same
reference as a page, and [integrators.md](../integrators.md) explains the socket API and its
compatibility promise.

### `leon api list`

List every method with its stability: `stable` (covered by the compatibility policy),
`experimental` or `internal`. `--json` gives `name`, `stability`, `description`, and `deprecated`
when the method is on its way out.

### `leon api describe METHOD`

Show a method's stability, description, JSON Schemas and, for the most-used methods, a request
and response example.

### `leon api call METHOD [ARGS]`

Call a method with JSON params (inline, `@file`, or `-` for stdin). `-s` fills `session_id`.
A block method isn't called here: it fails, pointing at `leon block call`.

```sh
leon api list
leon api describe session.view
leon api call server.status
```
