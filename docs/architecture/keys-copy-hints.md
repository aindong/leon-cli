# Architecture: repeat keys, copy mode and hints

tmux habits in the app and `leon attach`: repeatable bindings, copy mode and hints. Part of [the
architecture](../architecture.md).

## Decisions

- **Repeat keys, copy mode and hints (M19).** A binding with `repeats` (tmux `bind -r`) opens a
  window of `repeat_time` (default 500 ms) when it runs: a key that completes a repeatable binding
  after the same prefix runs it with no prefix and restarts the window; any other key closes it and
  is handled from no prefix (`keymap::Repeat`, shared by the app and `leon attach`). A repeated Lua
  function or config action goes to the server as `client.key {prefix, key}`, which resolves the
  whole sequence. Defaults never repeat. Key names ignore case (a capital is `shift+f`); shifted
  symbols match as typed or as `shift+<key>` (US layout). **Copy mode** (`client.copy_mode`, ⌘⇧C)
  is a cursor in absolute lines over the view and its cached scrollback (`leon_app::copy_mode`,
  GPUI-free): vi motions, `v`/`V` set the pane's selection (the server's, which takes both end
  cells, so `v` alone selects the one under the cursor), `/` and `?` run
  `dev.leon.terminal.find` and step from the cursor, `y` copies. Its badge or prompt is written
  over the pane's top-right cells (`stamp`) and the program's cursor is hidden; keys never reach
  the program, ⌘ chords still go to the keymap. `leon attach` has no copy mode: it draws only the
  live screen, with no scrollback view, cursor overlay or selection of its own. **Hints**
  (`client.hints`, ⌘⇧H) match `leon_core::hints` over the cells the pane shows: URLs, paths, git
  hashes, UUIDs, `0x` hex, IPv4, 4+ digit numbers, then the config's `hint_patterns` (at most 32,
  256 bytes each, compiled with a 1 MiB limit; Rust's regex is linear and matches don't overlap,
  so a screen yields at most a hint per cell). Labels are prefix-free (tmux-fingers' alphabet),
  shortest near the bottom, one per distinct text, at most 676; a label copies, and in upper case
  is pasted into the pane. A new size or screen labels the view again; copy mode ends when its
  lines are gone (an alt-screen switch, a full scrollback) and stays through a resize.
