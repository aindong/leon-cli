# Remote hosts

How leon works with the leon server on another machine, after the [README's](../README.md#work-on-your-devbox) first steps. The commands are in [cli/hosts.md](cli/hosts.md); to watch a machine's sessions from a browser over your tailnet, see [leon in the browser](web.md).

A dropped Wi-Fi link is retried for a minute: `leon events` and the app resume without losing or
repeating an event, and `leon attach` says so, then reconnects and redraws. Keys
typed while the link is stalled or down arrive whole or not at all, and the status bar says how many
were not sent (keys sent as a stall starts may land). Use ssh keys or an agent: each
connection is its own ssh session.

```mermaid
flowchart LR
  subgraph mac["This Mac"]
    client["Leon.app or the leon CLI"]
  end
  subgraph devbox["a host, such as devbox"]
    proxy["leon server proxy"] --> server["leon server"]
    listener["the tailnet listener,<br/>with serve_tailscale on"] --> server
  end
  client -->|"ssh://: an ssh session per connection"| proxy
  client -->|"tailnet://: straight over Tailscale"| listener
```

Who the tailnet listener lets in, step by step, is in
[Serving over Tailscale](architecture/tailnet.md#who-gets-in-step-by-step).

The app shows every host from `leon hosts` in the sidebar, under This Mac, each with its sessions
and tabs and a dot for its connection. Click a host's label to fold it away. A remote pane works
like a local one, with the host's name in its header. New Session, New Tab and the splits act on
the machine of the focused pane, and the command palette has a New Session action for each machine.

To add a host from the app, choose Add Host… in the command palette or in Settings › Remote Hosts.
When the Tailscale client is installed on this Mac, it first lists the machines on your tailnet, or
says why it can't, and picking one adds it as `leon hosts add --tailscale` would. Otherwise, or
when you pick "Another host by its ssh address…", it asks for a label (such as `devbox`) and then
for the `ssh://` address that reaches the host. The app saves the host to the same list `leon hosts` reads and starts connecting to it.

### Connect from the sidebar's footer

The sidebar's footer has two buttons: Connect on the left and Settings (⌘,) on the right. Connect,
also ⌘⇧K or "Connect to a Machine…" in the command palette, opens one menu for reaching another
machine; ↑, ↓ and ↩ work in it, and esc closes it:

- **Your hosts**: every host the app shows and every one `leon hosts` lists that it doesn't, each
  with its state (connected, connecting, unreachable, needs login, not shown). Choose a connected
  one to go to its sessions, one that isn't to try it again now, and one not shown to show it.
- **On your tailnet**: with Tailscale running here, your own other machines, read from the local
  Tailscale client's status (leon only reads it and changes nothing). Only machines signed in as
  you and not tagged are listed, since those are the only ones a `tailnet://` host reaches. Choose
  one to save it as `tailnet://NAME` under its short name and connect. leon doesn't probe them
  first: one that doesn't serve leon (`serve_tailscale`) shows unreachable.
- **Add a host…**: the Add Host prompt above.
- **This Mac's web page**: while the [web page](web.md) is served, "Open on another device" shows
  its address and a QR code to scan; else "Turn on in Settings…" opens Settings.

The app's ssh never asks for a password. A host that needs one says
`needs login: run leon hosts check devbox in a terminal`; do that once (accept the host key, unlock
your key), and the app's next try gets in. While a link is down the host's panes keep their last
screens under a "reconnecting" note.

![The sidebar with This Mac and a devbox, each with its sessions and tabs](images/m16b-hosts.png)

![A pane on the devbox, focused, with the host's name in its header](images/m16b-remote-pane.png)

![A host that can't be reached, with ssh's reason under its label](images/m16b-unreachable.png)

![Add Host… listing the machines on a tailnet, each with its system and whether it is online](images/app-tailnet.png)
