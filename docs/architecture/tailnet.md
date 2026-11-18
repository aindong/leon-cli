# Architecture: serving over Tailscale

leon's one network listener, off by default, and the `tailnet://` endpoints that reach it. Part
of [the architecture](../architecture.md); the ssh path it sits beside is in [Hosts](hosts.md).

## Decisions

- **Off by default, the local tailscaled's word, no embedded node.** `serve_tailscale` (a setting
  in init.lua) turns it on. The server asks the tailscaled already running on the machine, over
  its LocalAPI, and only `status?peers=false` and `whois` (never a change). leon doesn't embed a
  Tailscale node (tsnet is Go only, and a second node would need its own login and keys).
- **Finding the LocalAPI** (`leon_hosts::localapi`). Linux: `/var/run/tailscale/tailscaled.sock`
  (or `/run/…`); the open-source macOS daemon: `/var/run/tailscaled.socket`. A socket's peer must
  be root. The standalone Mac app: `/Library/Tailscale/ipnport` links to a localhost port and
  `sameuserproof-PORT` holds the token, sent as Basic auth; the dir, the link and the token file
  must be root's and not group- or other-writable, the link's target only digits, the token read
  with `O_NOFOLLOW`. The Mac App Store build (found through `lsof`) isn't supported: leon can't
  ask, so it lets no one in. Requests are `GET … HTTP/1.0` (a plain body that ends at the close),
  2 s each, at most 4 MiB (status) or 1 MiB (whois) read, and only a `200` counts. Every field is
  checked: the parsers keep raw ids and logins (never `shown_text`'s), a missing node, user id or
  login refuses, and `Tags` that is present but not a list of strings refuses.
- **Bound to the tailnet only.** The supervisor (`leon_server::tailnet`) looks at the settings and
  the status every 2 s. It binds `serve_tailscale_port` (default 7297, a number and never an
  address) on each of `Self.TailscaleIPs` that `direct::on_tailnet` accepts: inside
  100.64.0.0/10 or fd7a:115c:a1e0::/48, not 100.100.100.100 (Tailscale's DNS), not 4via6
  (`fd7a:115c:a1e0:b1a::/64`, LAN hosts behind a subnet router), not IPv4-mapped. Never `0.0.0.0`,
  `::` or loopback. A status with none of those binds nothing.
- **Who gets in** (`localapi::admit`), decided for every connection before a byte is read, on a
  blocking thread, within 3 s: status (still running and signed in as the pinned owner), whois
  of the caller's address, status again (the same node and owner). Then the caller must not be
  this machine itself (`Node.StableID == Self.ID`: another local user, or a program, dialing
  this machine's own tailnet address), and either be untagged with `UserProfile.ID` and
  `LoginName` equal to this machine's user id and login (`tagged-devices` never), or carry a tag
  listed in `serve_tailscale_tags` (exact match). No answer is cached. Anything else, tailscaled
  down, slow or answering an error included, closes the connection unread and unanswered, with one
  log line `leon: tailnet: refused ADDR: REASON` and the status's `last_refusal`.
- **The owner is pinned.** The first good status after a load that changed a `serve_tailscale*`
  setting fixes the account served (user id and login). A status that later says another account,
  or a tagged machine, stops serving with a reason (`Tailscale is now signed in as …`); only
  turning `serve_tailscale` off and on pins again, never an unrelated `leon config set`. A status
  call still out when the next 2 s tick comes is awaited again, not doubled.
  Stopped (off, logged out, tailscaled gone, switched, no address) closes the listeners and every
  connection they let in, and the server log and `server.status`'s `tailnet` say why.
- **Limits.** A caller outside the tailnet ranges is closed before any whois and leaves no
  entry. An address with 5 refusals in a minute is closed unasked for the rest of that minute
  (logged once); the count remembers 4096 addresses, and when full a new address is refused
  unasked. At most 32 connections (8 per address: a browser loads a page over 6) wait for a
  decision; more are refused. A failure of tailscaled's own doesn't count against the caller.
- **The same protocol.** A connection let in goes to `serve::connection`, the code the Unix socket
  uses (the stream halves are generic), when its first byte is `{`; a capital letter is HTTP, for
  [the web client](web.md) with `serve_web` on, and anything else is closed unanswered. The 1 MiB
  request line cap, events, resume and the idle count all apply. A caller can do what a local
  client can.
- **`tailnet://HOST[:PORT]`** is an `Endpoint` like `ssh://` (no user part; default port 7297),
  so `-H`, the hosts list, `leon attach` and the app reach it through the same `Link`: the link's
  Unix socket keeps its peer-uid check, and each connection, instead of ssh, is
  `direct::connect`: resolve the name, keep the addresses `on_tailnet` accepts, ask the local
  tailscaled's whois about each, `admit` it as the server would (one of your untagged machines,
  not this one), and connect to that checked address, from one of this machine's Tailscale
  addresses (else Tailscale went away meanwhile, and nothing is sent). A connection closed before any answer
  reads as "its server lets in only your own machines". No autostart.
- **Tests** run a fake LocalAPI (an HTTP server on a Unix socket in a private dir, answering
  status and whois from JSON the test sets) through `LEON_TEST_TAILNET =
  {"localapi": SOCKET, "loopback": true}`, which also lets loopback stand in for the tailnet. Only a
  debug build has code that reads it: `update_release_build` checks a release binary doesn't
  contain the name. The fake's socket must be ours. CI has no tailnet; the real path was checked by
  hand against the Mac app (status and whois only).

## Threat model

Before this, the only ways in were the Unix socket (peer uid checked) and ssh as the user. A
caller let in here can run anything as the user, so the rule is as strict as "may log in as me".

- **Strangers on the tailnet** (other members, shared-in nodes, tagged servers, anyone the ACL
  lets reach the port): whois names their account or tags, and they are refused before any byte
  is parsed. WireGuard authenticates source addresses on the tailnet, so one can't claim another
  machine's address.
- **The LAN or the internet**: the listener is bound only to Tailscale addresses. A neighbour
  routing to a 100.x address through another interface is refused before whois (its address
  isn't in the ranges) and costs no LocalAPI call.
- **Other users of the same machine**: they reach the Unix socket not at all (peer uid), and the
  tailnet listener only from this machine's own address, which whois answers with this machine's
  node: refused. They can bind the port first while leon isn't serving, and a client can't tell
  that from leon (it is the right machine): residual, on a shared machine leave this off.
- **Your other machines**: Tailscale identity is per machine. Anyone who can act on one of your
  untagged machines (another account there, a relay through `tailscale serve`, `socat`, ssh
  forwarding, an auth-key node you didn't tag, a subnet router or home gateway of yours that
  masquerades its LAN onto the tailnet) is "you". The docs say not to forward the port and to
  limit it to `autogroup:self` in the tailnet policy.
- **A fake LocalAPI**: socket paths need a root peer and the Mac app's files must be root's alone.
  On macOS, if tailscaled stops, a local user who binds its old localhost port gets the token and
  can answer whois, admit their own connection to this machine's address, and so get a shell as
  you: on a shared Mac this is privilege escalation, so leave serving off there. The Tailscale
  CLI trusts the same files the same way. Residual.
- **Account changes**: a switch, logout or stop is seen at the next connection (status before and
  after whois) and within 2 s by the supervisor, which closes the listener and the connections it
  let in.
- **Denial of service**: no protocol byte before the decision, 3 s per decision, 32 waiting (4 per
  address), per-address refusal limit, bounded LocalAPI reads; after admission the existing per-
  connection bounds apply.
- **The client**: `tailnet://` sends plain TCP, so it goes only to an address in the tailnet
  ranges that the local tailscaled says is your own untagged machine; a name that resolves
  elsewhere (CGNAT space while Tailscale is down, a LAN DNS answer) is refused before anything is
  sent, and a tailscaled that doesn't answer refuses it too.
- **Never**: Funnel, `tailscale serve`, or any `tailscale up/set/login/logout`; leon doesn't
  change Tailscale.
