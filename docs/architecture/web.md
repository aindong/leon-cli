# Architecture: the web client

leon's page in a browser, served over the tailnet listener to the owner's own machines. Part of
[the architecture](../architecture.md); the listener, `admit` and their threat model are in
[Serving over Tailscale](tailnet.md). The user's guide is [web.md](../web.md).

## Decisions

- **No new listener.** `serve_web` (off by default) does nothing without `serve_tailscale`: the
  page rides the tailnet listener, on its addresses and port, behind its `admit`, unchanged.
  Turning `serve_web` on or off rebinds, as any `serve_tailscale*` change does.
- **The first byte decides** (`leon_server::web::route`, after admission, by `peek`, nothing
  consumed): `{` is the NDJSON protocol, a capital ASCII letter is an HTTP request line, anything
  else is closed unanswered. HTTP with `serve_web` off is closed unanswered too, as any non-API
  connection is since #74. leon's own clients send `{` first.
- **One API.** `/ws` upgrades to a WebSocket whose text messages are request lines, fed through a
  64 KiB `tokio::io::duplex` to `serve::connection`, the function the Unix socket and the tailnet
  NDJSON use; each line it writes goes back as one text message. Methods, events, `block.watch`
  screens, the 1 MiB line cap and backpressure are the socket's own. No method is web-only.
- **HTTP, a small subset** (`web.rs`): `GET` only (else 405), a head of at most 8 KiB read within
  5 s (else closed) and 64 headers with token names (else 400), one request per connection
  (`Connection: close`), paths `/`, `/app.js`, `/conn.js`, `/grid.js`, `/app.css`, `/theme.css`,
  `/icon.svg` and `/ws` (else 404). A browser loads a page over up to 6 connections, each admitted
  on its own, so the listener lets 8 per address wait for a decision. The assets are
  plain JS modules and CSS in `crates/leon-server/web/`, built in with `include_str!`: no build
  step, no node, no framework (a few hundred lines don't pay for one), nothing from a CDN.
  `/theme.css` is leon-dark's colours, the palette `block.watch` resolves screens with by default.
- **Host first** (DNS rebinding). Every request needs exactly one `Host`, else 400, and it must be
  one this server answers to, else 421: each listening `ADDRESS:PORT` (`[v6]:PORT`), and the
  status's `Self.DNSName` with the port (case-insensitive, its trailing dot dropped; on port 80
  the port-less forms too). Not the short name: it resolves through search domains a LAN can
  influence. A rebinding page has its own name in `Host`, so it gets 421 and never sees the token.
- **Origin.** The upgrade's `Origin` must be `http://` + its checked `Host`, normalised the same
  way; missing, `null` or another is 403.
- **A token in the page, not a cookie.** `GET /` returns the page with a 256-bit token from
  `/dev/urandom` (a new one each time serving starts, so turning `serve_web` off and on revokes
  it) in `<meta name="leon-token">`, `Cache-Control: no-store`;
  the page opens `/ws?token=…`, and a missing or wrong token is 403 (compared in constant time,
  never logged). Another site can't read the page (no CORS headers) or frame it. A
  `SameSite=Strict` cookie was the other choice: cookies ignore ports, and the "site" of an IP host
  is the bare IP, so any other service on the machine, on any port, is same-site and both gets
  and can set the cookie.
- **Headers on every response:** `Content-Security-Policy: default-src 'none'; script-src 'self';
  style-src 'self'; img-src 'self'; font-src 'self'; connect-src 'self' ws://HOST; base-uri
  'none'; form-action 'none'; frame-ancestors 'none'`, `X-Frame-Options: DENY`,
  `X-Content-Type-Options: nosniff`, `Referrer-Policy: no-referrer`, `Cross-Origin-Opener-Policy`
  and `Cross-Origin-Resource-Policy: same-origin`. The page puts screen text in with
  `textContent` only, and styles through the CSSOM (which `style-src 'self'` allows).
- **The WebSocket codec** (`websocket.rs`): client frames must be masked; text and continuation
  frames, at most 1 MiB per message, counted from each frame header before its payload is kept
  (1009); binary 1003; a message that isn't UTF-8 or holds a newline 1007 (one message is one
  request line); control frames at most 125 bytes, pings answered. The decoder keeps a read
  offset, so many small frames cost linear time. Accept key: `sha1_smol` (in the tree already
  through `uuid`) and `base64`.
- **Status.** `server.status`'s `tailnet.web` lists the page's URLs while serving with
  `serve_web` (the MagicDNS one first), and `leon server status` prints the first.
- **HTTP only, for now.** tailscaled's LocalAPI `cert/<domain>` endpoint *issues* a certificate
  (an ACME order, which also puts the name in public Certificate Transparency logs) when none
  valid is cached, so leon must not call it. A cert file the user makes with `tailscale cert`
  and names in a setting would work, at the cost of a TLS library on Linux (rustls is in the tree
  only through GPUI) and a renewal every 90 days by hand; that waits on the user's call. Over
  HTTP the clipboard API and notifications are unavailable (the page isn't a secure context).
- **The page** (view only): sessions, the chosen session's tabs and the chosen tab's panes (the
  split tree, a zoomed pane, visible floats; not drawers yet), a refetch of `session.list`,
  `session.view` and `session.list_blocks` 50 ms after a burst of `session.attach_all` events,
  and `block.watch` for the panes shown. It never resizes a terminal: it scales the font to fit.
  On a closed connection it asks for `/` with a backoff up to 30 s and reloads once it answers,
  which fetches the new token.

## Threat model

Beyond [the listener's](tailnet.md#threat-model): the web adds a browser, which runs other
sites' code on the owner's own machine, and the owner's machines are exactly who `admit` lets in.

- **Cross-site WebSocket hijacking** (a page elsewhere opens `ws://100.x:7297/ws` from the owner's
  browser): no token, and a foreign Origin. Each is enough alone; tests remove each in turn.
- **Cross-protocol requests:** a connection that starts like HTTP never reaches the NDJSON
  reader.
- **DNS rebinding** (a name that turns into 100.x after the page loads): the Host is the
  attacker's name: 421 before the page or the WebSocket.
- **Framing and injection:** `frame-ancestors 'none'`, a CSP with no inline script, screen text
  as text.
- **Token leaks:** it is in the page (no-store) and the `/ws` URL, which no log line prints. A
  browser extension, or a program of the owner's on the browser's machine, can read it; such a
  program could already reach the port as the owner. Residual.
- **Not covered:** a browser of the owner's that is itself compromised.
