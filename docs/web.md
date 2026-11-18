# leon in the browser

Open your sessions in a web browser on another of your own machines, over your
[Tailscale](cli/hosts.md#serve-over-tailscale) network only. Part of [Remote hosts](remote-hosts.md).

Today the page is **view only**: it lists your sessions and each session's tabs, and draws the
chosen tab's panes live, in leon's colours and your `font.family` when the browser has that font.
Typing, copy and paste, and a phone layout come next.

## Turn it on

On the machine whose sessions you want to see:

```text
leon config set serve_tailscale true
leon config set serve_web true
leon server status       # web      http://devbox.example-tailnet.ts.net:7297/
```

Open that URL in a browser on another of your machines. `serve_web` does nothing unless
`serve_tailscale` is on: the page is served by the same listener, on the same port
(`serve_tailscale_port`), to the same machines. Turn it off with `leon config set serve_web false`.

Use the MagicDNS name or one of the machine's Tailscale addresses (`http://100.x.y.z:7297/`). The
short name (`http://devbox:7297/`) is refused on purpose; see below.

## Who gets in

- **Only your own machines**, by the rules in [cli/hosts.md](cli/hosts.md#serve-over-tailscale):
  before a byte is read, the server asks the local tailscaled who is calling and lets in only
  another machine signed in as you (or one with a tag you listed). Everyone else gets a closed
  connection.
- **Never anywhere else.** No public link, no relay, no Funnel. leon never changes Tailscale's
  settings and never asks it for a certificate. Nothing listens on your LAN, on `0.0.0.0` or on
  loopback.
- **Not a web page you happen to visit.** A page from another site, open in your own browser,
  could try to reach the server through your machine. It is stopped three ways:
  - every request must name this server (its MagicDNS name or address) as its Host;
  - the WebSocket the page uses must come from the page's own Origin;
  - it must carry a token that only the page holds (a fresh one each time the server starts),
    which another site can't read.

  Pages also carry a strict Content-Security-Policy, can't be framed, and load nothing from
  anywhere else: every file is built into leon.
- **What a page can do** is what a caller over the tailnet can do: the page speaks the same API as
  the socket. The page itself only reads today.

## HTTP, not HTTPS

The page is plain HTTP. Tailscale's WireGuard already encrypts and authenticates the link, so
nothing crosses the network in the clear. But browsers treat an `http://` page as not secure,
and some features need a secure page:

- the clipboard API (copy and paste buttons; ⌘C and ⌘V still work);
- browser notifications.

HTTPS needs a certificate for the machine's name. leon doesn't ask Tailscale for one, because
that creates a certificate (and publishes the name in public certificate logs).

## When it doesn't load

- `leon server status` shows no `web` line: `serve_tailscale` or `serve_web` is off, or Tailscale
  isn't running or signed in (the `tailnet` line says why).
- The page says "Disconnected": the server stopped or restarted, or serving stopped; the page
  reloads itself and picks up the new token.
- Nothing loads at all: the caller isn't one of your machines (the server log has a `refused`
  line naming the reason), or your tailnet policy blocks the port.
