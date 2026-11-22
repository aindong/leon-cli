# CLI: server

Part of the [command reference](../cli.md): running, starting, stopping and checking the server, and the proxy `-H` uses.

## Server

### `leon server`

Run, start, stop or check the server.

### `leon server run`

Run the server in the foreground. Sessions are saved in `$LEON_STATE_DIR` (default
`~/.local/state/leon`) while it runs and when it stops, and restored when it starts. After a crash
they come back as they were a second or two before it, with each pane's text from up to a minute
before. A restored block runs a fresh shell under the text it showed before, and offers its resume command if a program registered one;
nothing else that ran is run again.

- `--config PATH`: the Lua config (default as for `leon config check`).
- `--restore[=BOOL]`: restore the sessions the last server saved (default true).

### `leon server start`

Start a server in the background that keeps running until `leon server stop`, even with no
sessions and no client connected. A server that another `leon` command started on its own stops
once it has had no sessions and no client for 10 seconds. If such a server is running,
`leon server start` doesn't start a second one. It tells that server to keep running from then on.

- `--restore[=BOOL]`: restore the sessions the last server saved (default true).

### `leon server stop`

Stop the server. Every program in every session ends; the sessions are saved for the next start,
with the text each terminal shows (in the private state dir, 0600).

- `--discard`: don't save, and forget the sessions saved before, so the next start is empty.

### `leon server status`

Show whether a server runs; exit 4 if not. When [serving over
Tailscale](hosts.md#serve-over-tailscale) is on, a `tailnet` line says where it serves or why
it stopped, and another the last connection it refused. With the [web page](../web.md) on, a
`web` line gives its address.

- `--qr`: also draw the web page's address as a QR code (in half-block characters), to open it
  on a phone. On a terminal it is drawn white on black, so it scans in light and dark themes.

### `leon server proxy`

Connect stdin and stdout to this machine's server. Unless `--autostart=false`, it first does what
`leon server start` does, so the server keeps running after the ssh session ends. This is what
`leon -H` runs on the other machine over ssh; you don't run it yourself.
