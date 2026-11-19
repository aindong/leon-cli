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
- **A person at a keyboard, for the permission guard.** The guard
  ([attention.md](attention.md)) lets only a person answer a permission prompt, and counts another
  machine as one. The web client is always another machine (`admit` refuses this machine's own
  address), so both its WebSocket and the API on the same port reach `serve::connection` as
  `Peer::Remote`, as `tailnet://` callers do. That fits what the page is: it sends only what a
  person's keys, clicks and pastes in the browser make, never input of its own. Something that drives a browser on another of the owner's machines counts as a
  person too: the same residual as `tailnet://`.
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
- **HTTPS from the user's own `tailscale cert` files** (`web_tls.rs`). tailscaled's LocalAPI
  `cert/<domain>` endpoint *issues* a certificate (an ACME order, which also puts the name in
  public Certificate Transparency logs), so leon never calls it and never runs `tailscale cert`:
  the user makes the files and names them in `serve_web_cert` and `serve_web_key`. Over plain
  HTTP the clipboard API and notifications are unavailable (not a secure context).
  - **The same port:** after admission a first byte `0x16` (a TLS handshake) is TLS when HTTPS is
    set and a valid certificate is loaded, else closed; the handshake gets the head's 5 s. Inside,
    the same HTTP code runs; the Origin must be `https://HOST`, the CSP's socket source `wss:`.
    With HTTPS set, plain HTTP gets `308` to `https://HOST/…` after the Host check, whatever state
    the files are in: never the page, a token or a WebSocket.
  - **The files** (at most 1 MiB each) are looked at on the supervisor's 2 s tick: a stat (size,
    inode, modification and change times to the nanosecond) taken before reading, and a change
    reloads. Checks: a PEM chain and key, the key the user's own and not readable by others (on the
    open file), the key matching the certificate, the leaf covering the MagicDNS name
    (`rustls-webpki`), and its dates (a bounded DER walk to TBSCertificate's validity). A bad
    reload keeps the last good certificate while it is valid; expiry, checked on every tick, drops
    it. `server.status`'s `tailnet.web_error` says what is wrong and names the command, with
    `sudo` and `chown` on Linux, where only root or the operator may ask for a certificate.
    `tailnet.web` then lists only the hosts the certificate covers.
  - **rustls with ring.** rustls 0.23 and `ring` were already in Cargo.lock through GPUI's HTTP
    client on macOS; `ring` needs only a C compiler, where aws-lc-rs needs cmake and, on some
    targets, NASM, and is bigger. No default features: rustls `ring, std, tls12`, tokio-rustls
    `ring, tls12`, `rustls-webpki` `std` (the name check only). The release headless binary grew
    by 1.30 MiB, 11% (12,323,952 → 13,687,984 bytes, `--release --no-default-features`, macOS arm64).
- **The page** drives leon as the app does, with no method of its own:
  - **Keys**: `web/keys.js` maps a `KeyboardEvent` to a binding name (one `NAMES` table, which a
    test checks against the encoder), sent as `block.event {type: "key"}`; the server makes the
    bytes with `leon_core::keys::encode`, the app's and `leon attach`'s mapping, in the program's
    cursor-key mode at that moment. Text the keyboard composes goes through a hidden textarea's
    `input`/`compositionend` as `dev.leon.terminal.write`, so a key is sent once, never also as text.
    As in the app, a Mac's Option with a character types what it composes; elsewhere Alt is Meta;
    ctrl takes the key's place (`KeyboardEvent.code`), so it works on any layout.
  - **Paste and copy**: the textarea's `paste` event is `block.event paste` (bracketed by the
    program's mode); a drag selects through `dev.leon.terminal.selection` (absolute lines) and the
    `copy` event sets its text. Neither needs a secure context.
  - **Mouse**: `block.event mouse`/`wheel` at the cell, when the screen's `mouse` says the program
    asked and Shift isn't held; else the drag selects.
  - **Actions**: `session.create`/`destroy`, `new_window`/`close_window`/`focus_window`,
    `new_split`, `focus_block`, `block.close`; a × asks once.
  - **Size**: each shown pane's terminal is set to what its box fits at `font.size` through
    `dev.leon.terminal.resize`, only when the page's own layout of it changes, never because another
    client resized it (`leon attach`'s rule); the app, which re-fits whenever a screen's size
    differs from its pane, then wins, and the page draws that screen scaled down to its box.
  - **Activity**: `session.list_blocks` and `dev.leon.terminal.program_status` per block, in every
    session, at most once a second after events, ordered by when the page first saw each state;
    a click goes there and sends `block.event focus` (clears `done`/`error`). **Notifications** use
    the browser's API, only in a secure context and after the person allows them, for a hidden page.
  - The model is refetched (`session.list`, `session.view`, `session.list_blocks`) 50 ms after a
    burst of `session.attach_all` events (at least every 500 ms while they continue), with
    `block.watch` for the panes shown. On a closed connection it asks for `/` with a backoff up to
    30 s and reloads once it answers, which fetches the new token.

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
