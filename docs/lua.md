# Lua: config and scripts

leon speaks Lua 5.1 in two places:

- **The config**, `init.lua`, which the server loads at start: key bindings, modes and your own
  actions. Check it with `leon config check`; apply changes with `leon config reload`.
- **Scripts**, which `leon do` runs on your machine against the server: one-off automation, or a
  long-running watcher that reacts to events.

Both share the `leon.*` functions ([the list](lua/scripts.md#function-list)). Every example on
these pages is run by a test (`crates/leon/tests/m7b_lua_doc.rs`).

## Where the config lives

The server reads the first of: `server run --config PATH`, `$LEON_CONFIG`,
`$XDG_CONFIG_HOME/leon/init.lua`, `~/.config/leon/init.lua`. A missing file is an empty config.
`require` loads files next to it.

```sh
leon config check
leon config reload
leon keymap
leon actions
```

A config runs top to bottom on every load. A bad `leon.*` call is skipped with a `file:line`
message and the rest still loads; a syntax error fails the whole load, and the server keeps the
last good config. `leon config check` exits 1 in both cases.

## The pages

- [Keys and modes](lua/keys.md): `leon.bind`, sequences, repeat keys, a tmux-style preset,
  vim-navigator, modes and `leon attach`'s prefix, function bindings, floating panes and a
  scratch shell key.
- [Your own actions](lua/actions.md): `leon.action`, `leon.session.<method>` and `leon.call`.
- [Settings](lua/settings.md): every `leon.set` setting, its values and its default; a cursor
  shader file is one ([cursor-shaders.md](cursor-shaders.md)).
- [Scripts and the functions](lua/scripts.md): `leon do`, events, timers, `leon.exec`, the
  list of `leon.*` functions and the limits.

## Project sessions

See [Projects and git worktrees](recipes.md) for an example action and `leon do` script that
reuse a session per folder, with optional worktree sessions and an ignored-file setup hook.
