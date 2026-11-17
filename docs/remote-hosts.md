# Remote hosts

How leon works with the leon server on another machine, after the [README's](../README.md#work-on-your-devbox) first steps. The commands are in [cli/hosts.md](cli/hosts.md).

A dropped Wi-Fi link is retried for a minute: `leon events` and the app resume without losing or
repeating an event, and `leon attach` says so, then reconnects and redraws. Keys
typed while the link is stalled or down arrive whole or not at all, and the status bar says how many
were not sent (keys sent as a stall starts may land). Use ssh keys or an agent: each
connection is its own ssh session.

The app shows every host from `leon hosts` in the sidebar, under This Mac, each with its sessions
and tabs and a dot for its connection. A remote pane works like a local one, with the host's name in
its header; New Session, New Tab and splits act on the machine of the focused pane, and the palette
has New Session on each machine and Add Host… (your tailnet's machines, or a label, then an
`ssh://` endpoint). Click a host's label to fold it away. The app's ssh never asks for a password. A host that needs one says
`needs login: run leon hosts check devbox in a terminal`; do that once (accept the host key, unlock
your key), and the app's next try gets in. While a link is down the host's panes keep their last
screens under a "reconnecting" note.

![The sidebar with This Mac and a devbox, each with its sessions and tabs](images/m16b-hosts.png)

![A pane on the devbox, focused, with the host's name in its header](images/m16b-remote-pane.png)

![A host that can't be reached, with ssh's reason under its label](images/m16b-unreachable.png)

![Add Host… listing the machines on a tailnet, each with its system and whether it is online](images/app-tailnet.png)
