# CLI: hosts

Part of the [command reference](../cli.md): other machines, reached over ssh with `-H`, and the
list that names them.

## Reaching another machine

`-H HOST` reaches the leon server on another machine (a devbox, a cloud VM, another Mac) through
the system `ssh`, so `~/.ssh/config`, `ProxyJump`, keys and agents all work, and leon never sees a
password. Nothing listens on a network port (unless you turn on [serving over
Tailscale](#serve-over-tailscale)): leon runs `ssh -T … -- [USER@]HOST` and, on the
host, `leon server proxy` (found on `PATH`, else in `~/.local/bin`), which starts the host's
server if none runs. `LEON_SSH` names another ssh program. An endpoint is only
`ssh://[USER@]HOST[:PORT]` or `tailnet://HOST[:PORT]` with plain names; anything else is refused (exit 2). Each connection is
one ssh session (`leon attach` opens three), so use keys or an agent, and `ControlMaster auto` in
`~/.ssh/config` to share one login.

```text
leon -H ssh://me@devbox ls
leon hosts add devbox ssh://me@devbox
leon -H devbox attach
```

This machine's working directory means nothing on the host: with `-H`, `run`, `split` and `new`
start in the host's home unless `--cwd` gives an absolute path there (a relative one exits 2).

`leon server run|start|proxy`, `leon app`, `leon config check`, `leon skill`, `leon hosts` and
`leon self-update` are about this machine and refuse `-H`; `leon server status` and `leon server stop` work over it.

## `leon hosts`

Name other machines for `-H`, and check that they answer. The list is
`$LEON_STATE_DIR/hosts.json` (mode 0600, in the private state dir).

### `leon hosts list`

List the hosts; `--json` prints `[{"label", "endpoints", "icon"}]`. A host that is a machine on
your tailnet also shows who you are there (`tailnet: me@example.com`), and `offline` or `no
Tailscale SSH` when so; in JSON, `"tailnet": {"login", "online", "ssh"}`.

### `leon hosts add LABEL ENDPOINT…`

Add a host: a label (letters, digits, `.`, `_`, `-`) and its `ssh://` endpoints. `-H LABEL` uses
the first. A label is added once (exit 1 if it exists).

- `--icon NAME`: an icon name for the app to show.
- `--tailscale [MACHINE]`: add a machine from your tailnet instead (no endpoint). Without MACHINE,
  leon lists the tailnet's machines and reads a number or name; MACHINE is a MagicDNS name, or a
  short name only one machine has (exit 3 if none has it). See [Tailscale](#tailscale).

### `leon hosts remove LABEL…`

Remove hosts (exit 3 if one isn't there).

### `leon hosts check [LABEL…]`

Reach each host (default: every one) as a command would, and print `ok` with its leon version,
or why not. Exit 4 if any host failed.

## Tailscale

When Tailscale runs on this machine, `leon hosts add --tailscale` lists your tailnet's other
machines from the local client's `tailscale status --json` (the only Tailscale command leon runs;
it never changes your Tailscale settings):

```text
$ leon hosts add --tailscale
Machines on your tailnet (tailnet: me@example.com):
  1 buildbox.example-tailnet.ts.net  linux · online
  2 devbox.example-tailnet.ts.net    linux · online · Tailscale SSH
  3 laptop.example-tailnet.ts.net    macOS · offline
Add which (number or name)? 2
added devbox: ssh://devbox.example-tailnet.ts.net
```

The machine becomes an ordinary host: its short name as the label, `ssh://` and its MagicDNS name
as the endpoint (its Tailscale address when MagicDNS is off). ssh logs in with your local user name;
for another one, add the host by hand: `leon hosts add devbox ssh://me@devbox.example-tailnet.ts.net`.
If this machine doesn't use Tailscale's DNS (`--accept-dns=false`), MagicDNS names don't resolve
here: add the host by its `100.x` address instead (`tailscale status` shows it).

- **Tailscale SSH.** A machine marked `Tailscale SSH` lets you in with your tailnet identity: no
  keys to set up. For one without it, leon says so: run `tailscale set --ssh` on that machine, or
  set up an ssh key for it. Then run `leon hosts check LABEL` once in a terminal, which accepts
  the host key, or shows the sign-in link Tailscale SSH asks for when its policy says to check.
- **Finding Tailscale.** `LEON_TAILSCALE` names the program if set (a path that isn't there
  means no Tailscale); else `tailscale` on `PATH`;
  else, on macOS, `/Applications/Tailscale.app/Contents/MacOS/Tailscale`.
- **No Tailscale.** Not installed, stopped or signed out, `--tailscale` says so in one line and
  exits 1; everything else works as before.

## Serve over Tailscale

Off by default. With it on, this machine's server also listens on its Tailscale addresses (its
`100.x` IPv4 and `fd7a:115c:a1e0::` IPv6, never another interface), and another of your machines
reaches it directly, with no ssh and no sshd:

```text
leon config set serve_tailscale true          # on the machine to reach; its server must run
leon server start
leon server status                            # tailnet  serving on 100.64.0.5:7297, … for me@example.com
leon -H tailnet://devbox ls                   # on another of your machines
leon hosts add devbox tailnet://devbox.example-tailnet.ts.net
```

- **Who gets in**: only your own other machines. For each connection, before reading a byte,
  the server asks the local tailscaled (its LocalAPI `whois`) who is calling, and lets the call
  in only when that machine is signed in to Tailscale as the account this machine is signed in
  as, and isn't tagged. A tagged machine gets in only if `serve_tailscale_tags` lists one of its
  tags (`leon config set serve_tailscale_tags '["tag:ci"]'`). Anything else (another account, a
  shared-in machine, this machine itself, tailscaled down or slow) is closed with no reply, and
  the server log says why in one line (`leon server status` shows the last one). An address
  refused 5 times in a minute is closed unasked for a minute.
- **What a caller can do**: everything a local client can, since it is the same user on another
  machine: run commands, type into any pane, read every screen. Turn it on only where that is
  what you want.
- **Every machine signed in as you counts as you**: anyone who can use one of your untagged
  machines (another account on it, a program relaying the port with `tailscale serve`, `socat`
  or ssh port forwarding) can come in. Never forward the port, and limit it in your tailnet
  policy to your own devices, for example
  `{"action": "accept", "src": ["autogroup:member"], "dst": ["autogroup:self:7297"]}`.
- **The port** is `serve_tailscale_port` (default 7297): a number, never an address.
- **When Tailscale stops, logs out or switches account**, the server stops listening and says so
  (`leon server status`: `tailnet  stopped: …`). After a switch it serves again only once you turn
  `serve_tailscale` off and on, and then for the account it is signed in as then; no other
  setting change does that.
  Turning the setting off closes the listener and every connection it let in.
- **`tailnet://HOST[:PORT]`** (default port 7297) is a host endpoint like `ssh://`, for `-H`,
  `leon hosts add`, `leon attach` and the app. The client connects only to a Tailscale address
  that its own tailscaled says is one of your untagged machines; a name that resolves elsewhere
  is refused before anything is sent. There is no autostart: the far server must already run.
- leon reads tailscaled's LocalAPI (status and whois only, never a change): its socket on Linux,
  and the standalone Mac app's local port and token in `/Library/Tailscale`. The Mac App Store
  build isn't supported yet: with it leon can't ask and lets no one in.

The design and threat model are in
[docs/architecture/tailnet.md](../architecture/tailnet.md).
