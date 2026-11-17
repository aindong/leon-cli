# Lua: your own actions

Part of [Lua: config and scripts](../lua.md).

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
