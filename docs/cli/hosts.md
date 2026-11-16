# CLI: hosts

Part of the [command reference](../cli.md): other machines, reached over ssh with `-H`, and the
list that names them.

## Reaching another machine

`-H HOST` reaches the leon server on another machine (a devbox, a cloud VM, another Mac) through
the system `ssh`, so `~/.ssh/config`, `ProxyJump`, keys and agents all work, and leon never sees a
password. Nothing listens on a network port: leon runs `ssh -T … -- [USER@]HOST` and, on the
host, `leon server proxy` (found on `PATH`, else in `~/.local/bin`), which starts the host's
server if none runs. `LEON_SSH` names another ssh program. An endpoint is only
`ssh://[USER@]HOST[:PORT]` with plain names; anything else is refused (exit 2). Each connection is
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

- **Tailscale SSH.** A machine marked `Tailscale SSH` lets you in with your tailnet identity: no
  keys to set up. For one without it, leon says so: run `tailscale set --ssh` on that machine, or
  set up an ssh key for it. Then run `leon hosts check LABEL` once in a terminal, which accepts
  the host key, or shows the sign-in link Tailscale SSH asks for when its policy says to check.
- **Finding Tailscale.** `LEON_TAILSCALE` names the program if set; else `tailscale` on `PATH`;
  else, on macOS, `/Applications/Tailscale.app/Contents/MacOS/Tailscale`.
- **No Tailscale.** Not installed, stopped or signed out, `--tailscale` says so in one line and
  exits 1; everything else works as before.
