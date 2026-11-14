# Lua: config and scripts

leon speaks Lua 5.1 in two places:

- **The config**, `init.lua`, which the server loads at start: key bindings, modes and your own
  actions. Check it with `leon config check`; apply changes with `leon config reload`.
- **Scripts**, which `leon do` runs on your machine against the server: one-off automation, or a
  long-running watcher that reacts to events.

Both share the `leon.*` functions below. Every example on this page is run by a test
(`crates/leon/tests/m7b_lua_doc.rs`): the config examples through `leon config check`, the scripts
through `leon do -e`.

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

## Key bindings

`leon.bind(key, action[, args])` binds a key to an action. Keys are modifiers plus one key joined
by `+`: `cmd`, `ctrl`, `alt` (also `opt`), `shift`, and a character or `enter escape tab space
backspace delete left right up down home end pageup pagedown f1`–`f12`. Write keys unshifted
(`cmd+shift+=`, not `cmd++`). A **sequence** joins keys with `>`, like tmux's prefix key.

```lua
leon.bind("cmd+t", "client.tab.new")
leon.bind("ctrl+b>c", "session.new_window")
leon.bind("ctrl+b>%", "pane.split", { direction = "right" })
leon.bind("ctrl+b>\"", "pane.split", { direction = "down" })
leon.bind("ctrl+b>z", "pane.zoom")
```

**Today** the server resolves these bindings (`leon keymap` shows them, and the `client.key` API
method runs them), but the app doesn't send its keys through them yet: its own shortcuts are fixed
(see the README). Actions you define work everywhere, through `leon do`.

`leon.unbind(key)` removes a key's bindings, including the app's defaults, and returns how many it
removed. `leon.bindings()` lists the bindings made so far.

## Modes

A mode is a set of keys that only work while the mode is on. Enter it with `client.mode.enter`;
Escape leaves it. An **exclusive** mode swallows keys it doesn't bind, so stray typing doesn't
reach the terminal.

```lua
leon.mode("resize", { exclusive = true })
leon.bind("cmd+r", "client.mode.enter", { name = "resize" })
leon.bind("resize/left", "pane.resize", { direction = "left" })
leon.bind("resize/right", "pane.resize", { direction = "right" })
leon.bind("resize/up", "pane.resize", { direction = "up" })
leon.bind("resize/down", "pane.resize", { direction = "down" })
```

## Your own actions

`leon.action{ name, title, run, ... }` defines an action. Bind it to a key, run it from the shell
with `leon do NAME k=v…`, or call it from another action with `leon.invoke`. `run(ctx, args)` gets
the caller's context (`ctx.session_id`, `ctx.block_id`, `ctx.origin`) and the arguments; what it
returns is the result `leon do` prints. `error(msg, 0)` fails it.

Inside an action, `leon.session.<method>{…}` calls any session method of the server API with
`session_id` filled in (`leon api list` shows them all); `leon.call(method, params)` calls any
method by full name. Calls return the result, or `nil, err`: they never raise.

```lua
-- A log pane under the current one, a third of the height.
leon.action{
  name = "logs_below",
  title = "Open a log pane below",
  args = { file = "string?" },
  run = function(ctx, args)
    local file = args.file or "/var/log/system.log"
    local r, err = leon.session.new_split{
      anchor_id = ctx.block_id,
      direction = "vertical",
      side = "after",
      ratio = 0.33,
      layout = leon.layout.block{ options = { command = { "tail", "-f", file } } },
    }
    if not r then error(err, 0) end
    return r.block_id
  end,
}
leon.bind("cmd+shift+l", "logs_below")

-- Name the session after an argument: leon do name_session label=api
leon.action{
  name = "name_session",
  title = "Name this session",
  args = { label = "string" },
  run = function(ctx, args)
    return leon.session.set_label{ label = args.label }
  end,
}
```

Arguments are declared as `name = "type"` with types `string number integer boolean object array
any`, and a `?` suffix for optional ones.

## Function bindings

A key can run a Lua function directly. Return `true` if you handled the key; otherwise the key goes
on to the next binding, and finally to the terminal.

```lua
leon.bind("ctrl+x", function(ctx, ev)
  leon.session.set_label{ label = "busy" }
  return true
end)

-- Run a program and use its output. Actions run in the server's working directory, so the
-- caller says where: leon do branch_label dir="$PWD"
leon.action{
  name = "branch_label",
  title = "Label the session with the git branch",
  args = { dir = "string" },
  run = function(ctx, args)
    local r = leon.exec({ "git", "rev-parse", "--abbrev-ref", "HEAD" }, { cwd = args.dir, timeout = 2 })
    if not r.ok then error(args.dir .. " is not a git repo", 0) end
    leon.session.set_label{ label = (r.stdout:gsub("%s+$", "")) }
  end,
}
```

## Scripts

`leon do -e CODE` or `leon do script.lua` runs a script in the `leon do` process, on its own
connection to the server, so a script can never hang the server. It runs top to bottom, then stays
alive while it has event handlers or timers; `leon.stop([code])` ends it. `--for 10m` ends it after
a while.

```sh
leon do -e 'print(leon.session_id)'
leon do --for 1m ./watch.lua
```

A script acts on the session `-s` (or `$LEON_SESSION`) names; `leon.session_id` and
`leon.block_id` say which, and `leon.args` holds the `k=v` arguments.

**Read the layout:**

```lua script
local view = leon.session.view()
for _, w in ipairs(view.windows) do
  print(w.position, w.window_id, #w.block_ids .. " blocks", w.active and "(active)" or "")
end
```

**Run a command beside you and wait for it:**

```lua script
local r = assert(leon.session.new_split{
  anchor_id = leon.block_id,
  direction = "horizontal",
  side = "after",
  layout = leon.layout.block{ options = { command = { "sh", "-c", "echo building; exit 3" } } },
})
local ev = leon.wait("child_exited", { block_id = r.block_id }, 10)
print("the build exited with", ev and ev.payload.exit_code)
```

**React to events, with timers:**

```lua script
leon.on("child_exited", function(ctx, ev)
  print(ev.block_id, "exited with", ev.payload.exit_code)
end)
local ticks = 0
local t = leon.every(0.1, function()
  ticks = ticks + 1
end)
leon.after(0.5, function()
  t:cancel()
  print("ticked", ticks, "times")
  leon.stop()
end)
```

**Run programs:**

```lua script
local r = leon.exec({ "uname", "-s" })
print(r.ok, r.code, r.stdout)
local slow = leon.exec({ "sleep", "5" }, { timeout = 0.2 })
print("timed out:", slow.timed_out)
```

## Function list

| Function | Where | What it does |
|---|---|---|
| `leon.bind(key, action[, args])` / `leon.bind(key, fn)` | config | Bind a key, sequence (`a>b`) or mode key (`mode/key`). |
| `leon.unbind(key)` | config | Remove a key's bindings; returns the count. |
| `leon.mode(name[, {exclusive}])` | config | Declare a mode. |
| `leon.bindings([filter])`, `leon.modes()` | config | List bindings and modes made so far. |
| `leon.action{name, title, run, description?, args?}` | both | Define an action. |
| `leon.invoke(name[, args])` | both | Run an action (nested at most 16 deep). |
| `leon.session.<method>(params)` | both | Call a session method; fills `session_id`. |
| `leon.call(method[, params])` | both | Call any server method by full name. |
| `leon.block.call(creator, method[, args])` | both | Call a block method; fills `block_id`. |
| `leon.layout.block{options}`, `.horizontal(r, a, b)`, `.vertical(r, a, b)` | both | Build layouts for `new_split` and `new_window`. |
| `leon.exec(argv[, opts])`, `leon.exec.start(argv[, opts])` | both | Run a program and wait, or start it and get a handle. |
| `leon.json.encode(v)`, `leon.json.decode(s)`, `leon.log([level,] msg)` | both | JSON and logging. |
| `leon.on(event, fn)`, `leon.wait(event[, filter][, timeout])` | script | Handle or wait for events. |
| `leon.after(s, fn)`, `leon.every(s, fn)`, `leon.sleep(s)`, `leon.stop([code])` | script | Timers and ending. |

## Limits

`io.popen`, `os.execute` and `os.exit` are not available (use `leon.exec`), nor are C modules or
precompiled bytecode. On the server, a config load gets 5 seconds and an action 30 seconds.
`leon.exec` runs each program in its own process group, and a timeout signals the whole group.
