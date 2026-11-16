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
backspace delete left right up down home end pageup pagedown f1`–`f12`. A shifted symbol works as
typed or unshifted: `ctrl+b>%` and `ctrl+b>shift+5`, `cmd++` and `cmd+shift+=` are the same keys
(on a US layout). Key names ignore case, so a capital letter is written `shift+f`. A **sequence**
joins keys with `>`, like tmux's prefix key.

```lua
leon.bind("cmd+t", "client.tab.new")
leon.bind("ctrl+b>c", "client.tab.new")
leon.bind("ctrl+b>%", "pane.split", { direction = "right" })
leon.bind("ctrl+b>\"", "pane.split", { direction = "down" })
leon.bind("ctrl+b>z", "pane.zoom")
```

The app sends every key through these bindings, on top of its own defaults (the README's
shortcut table): a binding replaces the default on the same key, and a sequence whose first key
is a default turns that key into its prefix. While a sequence is half typed the app shows what can
follow; Escape, or a key that continues nothing, cancels it. Keys nothing binds go to the pane.
The app runs its own actions (`client.*`, `pane.*`, `session.*`, listed in its command palette,
⌘⇧P); your actions and function bindings run in the server. `leon keymap` shows what is bound.

`leon.unbind(key)` removes a key's bindings, including the app's defaults, and returns how many it
removed. `leon.bindings()` lists the bindings made so far.

### Repeat keys

A 4th argument `{ repeats = true }` makes a sequence repeatable, like tmux's `bind -r`: after
`ctrl+b>h` runs, `h` alone runs it again for `repeat_time` (500 ms unless you set it), and each
repeat starts the time over. Any repeatable key after the same prefix works in that window, so
`ctrl+b h h l` goes left twice and right once. Another key ends the window and does what it would
have done with no prefix typed. (`repeat` is a Lua keyword, so the option is `repeats`;
`["repeat"] = true` works too.) The app's default keys don't repeat. A repeatable Lua function
runs again each time, the same as when its whole sequence is typed.

```lua
leon.bind("ctrl+b>h", "pane.focus.left", nil, { repeats = true })
leon.bind("ctrl+b>l", "pane.focus.right", nil, { repeats = true })
leon.set("repeat_time", 400)
```

### A tmux-style preset

Prefix `ctrl+b`, then the key tmux uses. `hjkl` repeat, so `ctrl+b h h h` walks three panes left.
`ctrl+b [` is copy mode (⌘⇧C in the app) and `ctrl+b F` hints (⌘⇧H). In `leon attach` the same
sequences come after its prefix, except copy mode and hints, which only the app has so far.

```lua
-- tmux-style keys: ctrl+b, then tmux's key
leon.bind("ctrl+b>%", "pane.split.right")
leon.bind("ctrl+b>\"", "pane.split.down")
leon.bind("ctrl+b>h", "pane.focus.left", nil, { repeats = true })
leon.bind("ctrl+b>j", "pane.focus.down", nil, { repeats = true })
leon.bind("ctrl+b>k", "pane.focus.up", nil, { repeats = true })
leon.bind("ctrl+b>l", "pane.focus.right", nil, { repeats = true })
leon.bind("ctrl+b>z", "pane.zoom")
leon.bind("ctrl+b>x", "pane.close")
leon.bind("ctrl+b>c", "client.tab.new")
leon.bind("ctrl+b>n", "client.tab.next")
leon.bind("ctrl+b>p", "client.tab.previous")
leon.bind("ctrl+b>[", "client.copy_mode")
leon.bind("ctrl+b>shift+f", "client.hints")
leon.set("repeat_time", 500)
```

### Moving between vim and leon (vim-navigator)

The same ctrl+h/j/k/l move between vim's windows and leon's panes. leon's side is a function
binding: when vim (or nvim) is the pane's foreground program the key goes on to vim, else leon
moves the focus. It asks `ps` for the pane's terminal, about 10 ms a key.

```lua
-- vim-navigator: ctrl+h/j/k/l go to vim when vim or nvim is in front, else to leon's panes
local function vim_in_front()
  local p = leon.block.call("dev.leon.terminal", "process", {})
  if not p then return false end
  local r = leon.exec({ "ps", "-o", "tty=", "-p", tostring(p.pid) }, { timeout = 1 })
  local tty = r.ok and r.stdout:match("%S+")
  if not tty then return false end
  r = leon.exec({ "ps", "-o", "stat=,args=", "-t", tty }, { timeout = 1 })
  for stat, program in (r.stdout or ""):gmatch("(%S+)%s+(%S+)[^\n]*") do
    local name = program:match("([^/]+)$")
    if stat:find("+", 1, true) and (name == "vim" or name == "nvim" or name == "vi") then
      return true
    end
  end
  return false
end

for key, dir in pairs{ h = "left", j = "down", k = "up", l = "right" } do
  leon.bind("ctrl+" .. key, function(ctx)
    if vim_in_front() then return false end -- not handled: the key goes on to vim
    leon.session.focus_direction{ direction = dir }
    return true
  end)
end
```

vim's side moves between its windows and, at an edge, runs `leon focus`. Every leon pane has
`LEON_BLOCK` and `LEON_SESSION` set, so `leon focus left` moves in the pane's own session, and
outside leon the mapping stays inside vim:

```vim
" ~/.vimrc: at the edge of vim's windows, hand the move to leon
function! s:Go(key, dir) abort
  let l:was = winnr()
  execute 'wincmd ' . a:key
  if winnr() == l:was && !empty($LEON_BLOCK)
    call system('leon focus ' . a:dir)
  endif
endfunction
nnoremap <silent> <C-h> :call <SID>Go('h', 'left')<CR>
nnoremap <silent> <C-j> :call <SID>Go('j', 'down')<CR>
nnoremap <silent> <C-k> :call <SID>Go('k', 'up')<CR>
nnoremap <silent> <C-l> :call <SID>Go('l', 'right')<CR>
```

## Modes

Modes are read by the server, but neither the app nor `client.key` acts on them yet (`leon attach`
uses one, `attach`, below). A mode is a
set of keys that only work while the mode is on. Enter it with `client.mode.enter`; Escape leaves
it. An **exclusive** mode swallows keys it doesn't bind, so stray typing doesn't reach the
terminal.

```lua
leon.mode("resize", { exclusive = true })
leon.bind("cmd+r", "client.mode.enter", { name = "resize" })
leon.bind("resize/left", "pane.resize", { direction = "left" })
leon.bind("resize/right", "pane.resize", { direction = "right" })
leon.bind("resize/up", "pane.resize", { direction = "up" })
leon.bind("resize/down", "pane.resize", { direction = "down" })
```

### `leon attach`'s prefix

`leon attach` (the terminal client) reads the mode named `attach`: its `prefix` is the key that
starts a command (default `ctrl+b`; `leon attach --prefix KEY` overrides it), and its bindings are
the keys after it. They replace or add to the built-in ones (`d` detach, arrows and `hjkl` focus,
`%` and `"` split, `z` zoom, `c` new tab, `n`/`p` next and previous tab, `s` the picker);
`leon.unbind("attach/KEY")` removes one. A sequence that starts with the prefix (`ctrl+b>c` above)
comes before a built-in key, and an `attach/` binding before both. The app ignores the mode.

```lua
leon.mode("attach", { prefix = "ctrl+a" })
leon.bind("attach/x", "pane.close")
leon.bind("attach/t", "client.tab.new")
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

## Settings

`leon.set(name, value)` sets a setting. init.lua is the only place settings live; `leon config
get` and `set` ([cli/settings.md](cli/settings.md)) and the app's Settings window (⌘,) read and edit
them in place. A running app follows every one of them when the config reloads (`leon config set`,
the Settings window and `leon config reload` all reload it). A bad value is a skipped call with a `file:line`
message, and the setting keeps its default.

| Setting | Takes | Default |
|---|---|---|
| `theme` | a theme name, or `{ dark = NAME, light = NAME }` to follow the OS appearance | `"leon-dark"` |
| `font.family` | a font's name | `"Menlo"` |
| `font.size` | points, 8 to 32 | `13` |
| `notifications.done`, `notifications.blocked`, `notifications.error` | `true` or `false` (below) | `true` |
| `copy_on_select` | `true` or `false` | `false` |
| `repeat_time` | milliseconds, 0 (off) to 10000 | `500` |
| `hint_patterns` | a list of regular expressions, at most 32 | `{}` |
| `resume` | `"ask"`, `"always"` or `"never"` | `"ask"` |
| `fade_inactive_panes` | a number, 0 (off) to 0.8 | `0.35` |
| `check_for_updates` | `true` or `false` ([updates.md](updates.md)) | `true` |

`theme` names `leon-dark`, `leon-light` or a theme `leon theme import` stored; an unknown name is
an error.

```lua
leon.set("theme", { dark = "leon-dark", light = "leon-light" })
leon.set("font.family", "Menlo")
leon.set("font.size", 14)
leon.set("notifications.done", false)
leon.set("check_for_updates", false)
```

`resume` is what a block restored after a server restart does with the resume command its program
registered for itself (`set_resume`, which leon's agent hook calls for Claude Code and Codex).
`"ask"` types the command at the new prompt and waits for Enter; `"always"` runs it; `"never"`
leaves the prompt empty. The server reads it at start, before it restores (it waits at most a
second for the config, and past that offers nothing, as with `"never"`); `leon config check` prints it.

```lua
leon.set("resume", "always")
```

`copy_on_select` makes the app copy a selection to the clipboard as soon as you make it, without
⌘C.

```lua
leon.set("copy_on_select", true)
```

`fade_inactive_panes` is how far the app fades the panes without the focus toward the window's
background; a status indicator and a pane that needs you never fade.

```lua
leon.set("fade_inactive_panes", 0)
```

`repeat_time` is how long a repeatable binding's keys work without the prefix (above; 0 turns
repeat off).

`hint_patterns` adds regular expressions (Rust syntax) to what the app's hints label. Hints
(`client.hints`, ⌘⇧H) put a short label on every URL, path, git hash, UUID, `0x` number, IPv4
address and number of four or more digits the pane shows; type a label to copy its text, or type it
in upper case to type the text into the pane. A pattern with a group labels what the group matched.
At most 32 patterns, each at most 256 bytes; a bad one is refused when the config loads.

```lua
leon.set("hint_patterns", { "[A-Z]+-\\d+", "sha256:([0-9a-f]{12})" })
```

`client.copy_mode` (⌘⇧C) is the app's keyboard copy mode over the scrollback, with vi keys:
`h j k l`, `w b`, `0 $`, `g G`, ctrl+u and ctrl+d move; `v` selects characters and `V` lines; `/`
and `?` search down and up, `n` and `N` step; `y` (or Enter) copies the selection and leaves; Esc
or `q` leaves.

The app notifies you (macOS banners, only from the bundled Leon.app) when a block becomes blocked
(`notifications.blocked`), reports an error (`notifications.error`), or finishes a run of 30 s or
more while the app is in the background (`notifications.done`). Blocked and error don't notify for the pane you are looking at, a block notifies at most
once a minute per state, and at most four notices go out a minute in all (the rest come as one).

```lua
leon.set("notifications.done", false)
```

The server reports the effective settings, defaults filled in and `theme` always as
`{ dark, light }`, with `server.settings`; a reload that changes any of them sends `--all`
subscribers a `settings_changed` event carrying the same object. A script can read them:

```lua script
local r = assert(leon.call("server.settings"))
assert(r.settings.font.size >= 8 and r.settings.theme.dark ~= nil)
```

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
| `leon.mode(name[, {exclusive, prefix}])` | config | Declare a mode; `prefix` (one key) is `leon attach`'s for the `attach` mode. |
| `leon.bindings([filter])`, `leon.modes()` | config | List bindings and modes made so far. |
| `leon.set(name, value)` | config | One of the [settings](#settings). |
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

## Project sessions

See [Projects and git worktrees](recipes.md) for an example action and `leon do` script that
reuse a session per folder, with optional worktree sessions and an ignored-file setup hook.

## Floating panes and a scratch shell key

`leon.float{block_id=...}` floats the current block when `block_id` is omitted;
`leon.float{new=true, layout={block={label="notes"}}}` starts a new floating shell.
Both accept `bounds={x=0.1,y=0.1,width=0.8,height=0.8}`. `leon.dock{side="right",
size=0.25}` pins the current block; sides are `left`, `right`, `bottom`.
`leon.toggle{label="scratch",create=true}` creates a bottom drawer on first use,
then hides/shows it. Hidden programs keep running. These helpers fill `session_id`
from context and return the API result or `nil, err`. To return a float to the
split tree use `leon.session.unfloat_block{block_id=...}`.

Put this in init.lua for a scratch shell on Ctrl+G (the recipe is tested):

<!-- m20-scratch -->
```lua
leon.action{
  name = "scratch_shell",
  title = "Toggle scratch shell",
  run = function()
    local result, err = leon.toggle{label="scratch", create=true}
    if not result then error(err, 0) end
    return result
  end,
}
leon.bind("ctrl+g", "scratch_shell")
```
<!-- /m20-scratch -->

The label must be unique within the session. Creation fails if the bottom side
already has a drawer. Existing programs and their visibility restore with saved
sessions, using the usual fresh-shell/resume policy after server restart.

Attach also accepts the client action names `pane.float` and `pane.new_floating` in
`leon.bind`; their GUI counterparts are part of M20b.
