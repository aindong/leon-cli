# Architecture: other machines

Remote hosts over ssh, resuming a dropped link, and the app's hosts; persistence and packaging are
in [State, packaging and hosts](state-and-hosts.md). Part of [the architecture](../architecture.md).

## Decisions

- **Remote hosts.** `-H HOST` (a label from `leon hosts`, or `ssh://[USER@]HOST[:PORT]`) opens a
  `Link` (`leon-hosts`): a Unix socket in a fresh private dir, `link-XXXXXX/s` under the runtime
  dir (0700, socket 0600), served by threads in the client process. Each connection to it spawns
  `$LEON_SSH` (default `ssh`) as argv, never through a local shell:
  `-T -a -x -o ClearAllForwardings=yes -o ServerAliveInterval=15 -o ServerAliveCountMax=3
  -o ConnectTimeout=15 [-p PORT] -- [USER@]HOST CMD`, and copies bytes both ways, only for a peer of our uid (as the server checks). So every client that takes a socket path works unchanged over ssh: the CLI,
  `leon do` scripts, and `leon_app::remote::Remote` (`leon attach`, and the app). leon adds
  only `-T`, no forwarding (no agent on the host, no port a config's `LocalForward` would bind) and
  the keepalives (a dead Wi-Fi link is noticed in about 45 s) and the connect timeout (a command
  started during an outage fails in 15 s, a stalled banner included); everything else comes from
  `~/.ssh/config`, and passwords stay between ssh and the terminal. Once `leon attach` owns the
  terminal it calls `Link::never_prompt`: later ssh runs get `-o BatchMode=yes`, so a reconnect
  fails and retries instead of reading a password through the TUI's raw keys. CMD is one fixed string
  with nothing from the endpoint in it: `sh -c '…'` finds `leon` on PATH, else
  `$HOME/.local/bin/leon`, else exits 127, which the link reports as "install leon on HOST".
  There `leon server proxy` does what `leon server start` does (a persistent server, restoring
  saved sessions) and bridges stdin and stdout to `server.sock`; the server's peer-uid check holds
  because ssh runs the proxy as the host's user. Only a link's connections until one reaches the
  server may autostart; later ones pass `--autostart=false`, so a server stopped on purpose stays
  stopped. Nothing listens on TCP. An endpoint is parsed into words of `[A-Za-z0-9._-]` (or an
  IPv6 address in brackets) that can't start with `-`, a port 1–65535, and no path: anything that
  could reach ssh as an option or a shell as code is a bad command line (exit 2). The hosts list is
  `$LEON_STATE_DIR/hosts.json` (`{"version":1,"hosts":[{label, endpoints, icon}]}`, spec §2),
  written 0600 via a temp file and a rename, read only from a private dir and a regular file of
  ours, and each endpoint is parsed again when used. With `-H`, `LEON_SESSION`/`LEON_BLOCK` are
  ignored. A link flocks `lock` in its dir while it lives; dropping it ends its accept loop (a
  connection of its own wakes it) and so closes the socket, since the app opens one per connection
  attempt for as long as it runs; a killed client leaves its dir, and the
  next `Link::open` removes `link-*` dirs whose lock it can take (never by connecting: a live link
  answers a connection by running ssh). One ssh session per connection is the price of the simple design
  (`leon attach` opens three); `ControlMaster auto` shares one login.
- **Resume and reconnect.** `session.attach`/`session.attach_all` with `resumable: true` also
  return a `resume` token. Such a stream keeps the last 1 MiB of lines it wrote (taken off the
  queue before the write, so a line lost in a dying pipe is kept). When its connection ends it is
  parked for 60 s, still queueing (the 8 MiB drop still applies). `session.resume {resume, seq}`
  on a connection that hasn't attached moves the queue there under the state lock and puts the
  kept lines after `seq` in front: every event after the last one the client saw comes once, with
  the same seq counter. A resume while the old connection is still open (a half-dead link) takes
  the stream over and the old connection ends. The token lasts for every later drop. An unknown
  or expired token, a `seq` ahead of the stream or older than the kept lines fails with
  `resume_failed` (410). `leon events -H` attaches resumable and, when the link drops, reconnects
  with backoff (100 ms doubling to 5 s, for up to 60 s) and resumes; refused (the server
  restarted), it attaches again and prints a new snapshot line. `leon attach -H` and the app use
  `Remote::connect_retrying` (`leon_app::remote`, the link parts in `remote_link.rs`): the event
  stream is attached resumable, and when a connection drops the reader reopens the fetch, call and
  event connections with the same backoff, then `session.resume`s after the last seq it saw (every
  event once, in order; only a refused resume attaches afresh), refetches the model and re-watches
  the screens (a watch set is per connection). A half-dead link (lost Wi-Fi, a stopped proxy)
  closes nothing, so a watchdog pings an event connection that has been quiet for 5 s and shuts it
  down when 5 s more bring no line: about 15 s to notice, not ssh's 45. Over a link the client's
  calls that nobody waits for (keys, resizes, the focus event, pastes) and layout changes (`act_later`:
  the call, the refetch, then a callback) run in order on one queue thread over the call
  connection, so a slow or stalled link never holds the thread that asked; while the link is down
  they fail at once. `leon attach` sends keys whole or not at all (`attach/typing.rs`): a write sent
  into a stalled link may land long after, over the old ssh connection and behind anything sent on
  the new one, so after a second with no answer on the call connection keys wait (up to 4 KiB)
  behind a `server.version` probe and go in one write when it answers; a failed probe or write
  drops every key waiting, as does a key typed while the link reconnects, and the bar counts the
  keys not sent. A stall that starts within that second can still catch the one write then in
  flight; ruling that out needs writes the server can tell apart from a replaced connection's.
  While attached, stderr is
  /dev/null, so ssh's errors during a reconnect aren't shown. Instead `leon attach`'s status bar
  says `reconnecting to HOST (try N)…` while the client relinks (it wakes the TUI on each try), and
  when the host's hold runs out it exits naming the link, not the server, which may be fine.
- **The app's hosts.** `leon_app::hosts` holds the machines the window shows: This Mac (the
  socket the app started with), then each host from the hosts list, read at start and after Add
  Host. Each host is connected on a thread of its own: a `Link` (with `never_prompt`, so ssh runs
  with `BatchMode=yes`: the app has no terminal to type a password into) and `connect_retrying`
  for 60 s, the server's resume hold. A link keeps the last line ssh wrote on stderr
  (`Link::last_error`), which is the reason shown: a host is *connecting* until its first attempt
  ends, *connected*, *reconnecting* while its remote retries (the try and the wait), *unreachable*
  with ssh's words, or *needs login* when BatchMode refused a password, passphrase or host key, with
  the advice to run `leon hosts check LABEL` in a terminal (which uses the same ssh, so once it
  works, an agent or a ControlMaster carries the app's next try). A host that gave up is tried again
  after 2 s, doubling to 60 s. The app never opens a pane that runs ssh for a login: that would be a
  second connection that teaches nothing about the app's own. The sidebar groups sessions under
  their machine's label (a dot for the state, a note under it while not connected; collapsed hosts
  saved in app.json as `collapsed_hosts`), only once a host exists, so with none it looks as it
  did. The window's `app` is the selected session's machine's remote, so New Session, New Tab,
  splits, keys and the palette act on the focused pane's host; leaving a host unwatches its panes.
  While a host reconnects its panes keep their last screens under a stale overlay. Lua bindings
  and config actions run on This Mac's server, the keymap's owner, with a host's panes they get no
  ids (shortcut). The palette lists New Session on each connected machine (`session.new {host}`)
  and Add Host… (`client.host.add`): a label, then an endpoint, each checked as `leon hosts add`
  checks them (`leon_hosts::hosts::add`), and saved through the hosts API.
