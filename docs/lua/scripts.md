# Lua: scripts and the functions

Part of [Lua: config and scripts](../lua.md).

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
| `leon.mode(name[, {exclusive, prefix}])` | config | Declare a mode; `prefix` (one key) is `leon attach`'s for the `attach` mode. |
| `leon.bindings([filter])`, `leon.modes()` | config | List bindings and modes made so far. |
| `leon.set(name, value)` | config | One of the [settings](settings.md). |
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
