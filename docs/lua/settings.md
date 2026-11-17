# Lua: settings

Part of [Lua: config and scripts](../lua.md).

## Settings

`leon.set(name, value)` sets a setting. init.lua is the only place settings live; `leon config
get` and `set` ([cli/settings.md](../cli/settings.md)) and the app's Settings window (⌘,) read and edit
them in place; a running app follows them on a reload. A bad value is a skipped call with a
`file:line` message, and the setting keeps its default.

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
| `check_for_updates` | `true` or `false` ([updates.md](../updates.md)) | `true` |
| `translucency` | a number, 0 (opaque) to 0.8 | `0.3` |
| `serve_tailscale` | `true` or `false` ([cli/hosts.md](../cli/hosts.md#serve-over-tailscale)) | `false` |
| `serve_tailscale_port` | a port, 1 to 65535 | `7297` |
| `serve_tailscale_tags` | a list of tags such as `"tag:ci"`, at most 16 | `{}` |

`theme` names a theme `leon theme list` shows (built in or imported); an unknown name is an error.

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

`fade_inactive_panes` is how far the app fades the panes without the focus toward the window's
background; a status indicator and a pane that needs you never fade.

```lua
leon.set("fade_inactive_panes", 0)
```

`translucency` is how much desktop shows through Leon.app, blurred, tinted by the theme.
Text keeps 4.5:1 over any desktop (muted 3:1), or the theme's own if lower.
Reduce Transparency and full screen are opaque, as is `leon.set("translucency", 0)`.

`serve_tailscale` makes the server also listen on this machine's Tailscale addresses, on
`serve_tailscale_port`, for your own other machines only (and tagged ones with a tag in
`serve_tailscale_tags`); [cli/hosts.md](../cli/hosts.md#serve-over-tailscale) has the rest.

`repeat_time` is how long a repeatable binding's keys work without the prefix ([keys.md](keys.md#repeat-keys)).

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

The server reports the effective settings, defaults filled in and `theme` always as
`{ dark, light }`, with `server.settings`; a reload that changes any of them sends `--all`
subscribers a `settings_changed` event carrying the same object. A script can read them:

```lua script
local r = assert(leon.call("server.settings"))
assert(r.settings.font.size >= 8 and r.settings.theme.dark ~= nil)
```
