# CLI: layout

Part of the [command reference](../cli.md): moving the focus and rearranging a window's panes.

## Layout

### `leon focus TARGET`

Focus a block, or move focus `left`, `right`, `up`, `down`, `next` or `prev` in the active window.

- `--wrap`: with nothing in that direction, go round to the far side.

### `leon move [BLOCK] DIRECTION ANCHOR`

Put a block (default: the focused one) beside `ANCHOR`, to its `right`, `left`, `above` or
`below`. Both must be in the same session; they may be in different windows.

- `--ratio PCT`: the moved block's share of the space (default 50).

### `leon swap BLOCK OTHER`

Two blocks trade places.

### `leon zoom [BLOCK]`

Show a block filling its window, or stop. Toggles by default.

- `--on`: zoom in, even if already zoomed.
- `--off`: zoom out, even if not zoomed.

### `leon resize [BLOCK]`

Set a block's share of the split it is in.

- `--ratio PCT`: the block's share, 1–99 (required).

```sh
leon focus left
leon move logs below tests --ratio 30
leon zoom -b tests --on
leon swap logs tests
leon resize -b logs --ratio 25
```
