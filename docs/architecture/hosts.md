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
  `leon do` scripts, and `leon_app::remote::Remote` (`leon attach`, and the app). That ssh gets its stdin, stdout and stderr and no
  other descriptor of the client's: every other one is marked close-on-exec in the forked child, because macOS marks a new
  pipe in a second step, and an ssh forked in between (by another connection's thread) held that connection's pipes, so
  its `spawn` never returned. A `leon.exec` program starts the same way (`leon_core::spawn::stdio_only`), so it can't hold a pane's
  PTY that another thread is opening. A pane's program starts the same way as well: leon opens the PTY
  with `portable-pty` but forks and execs the program itself (`terminal_start.rs`), since
  portable-pty's spawn closes every descriptor past stderr, std's exec-error pipe among them, and
  a failed exec then aborted in the pane instead of failing `block.create`. leon adds
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
  `$LEON_STATE_DIR/hosts.json` (`{"version":1,"hosts":[{label, endpoints, icon}]}`),
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
  calls that nobody waits for (keys, resizes, mouse and wheel reports, the focus event, pastes) and
  layout changes (`act_later`: the call, the refetch, then a callback) run in order on one queue
  thread over the call connection, so a slow, stalled or hostile link never holds the thread that
  asked; while the link is down they fail at once. The app's calls whose reply it uses (history
  rows, `find`, `link_at`, a word selection's ends) go through that queue too, one of each kind out
  at a time, and the next poll applies the reply (a copy-mode motion into rows not fetched yet sees
  them on its next try); other selections are worked out in the app. Every call has one deadline
  for its write and its reply (5 s; a Lua call 35 s), which lines that aren't the reply don't
  extend, and a watch line waits at most 100 ms. A copy from a host's pane is the text of the cells
  the app drew over the range dragged, never the host's `selection` text, and a host's link opens
  without asking only when its uri is the text on screen there. `leon attach` sends keys whole or
  not at all (`attach/typing.rs`): a write sent into a stalled link may land long after, over the old ssh connection and behind anything sent on
  the new one, so after a second with no answer on the call connection keys wait (up to 4 KiB)
  behind a `server.version` probe and go in one write when it answers; a failed probe or write
  drops every key waiting, as does a key typed while the link reconnects, and the bar counts the
  keys not sent; after a drop, keys go on dropping until the call that was out answers, and a paste
  waits in the same line as the keys. A stall that starts within that second can still catch the one write then in
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

## Tailscale

- **Discovery, not a new kind of host.** `leon hosts add --tailscale` and the app's Add Host… read
  the local client's `tailscale status --json`, the only Tailscale command leon runs (no `up`,
  `set` or `login`: leon never changes the user's Tailscale). A chosen machine is written to
  `hosts.json` as an ordinary host (label: the first label of its MagicDNS name; endpoint:
  `ssh://` and that name, or its first Tailscale address, IPv4 first, when
  `CurrentTailnet.MagicDNSEnabled` is false), so links, resume, attach and the sidebar don't know
  where it came from. Who a host is on the tailnet is worked out each time it is shown, by
  matching the endpoint's host to a peer's MagicDNS name or address; nothing about Tailscale is
  stored. `leon hosts list` and the app read the status only when there are hosts, and the app off its
  UI thread, at start and when Add Host… opens.
- **Finding the client.** `$LEON_TAILSCALE` names it (like `LEON_SSH`, for tests and odd installs);
  else an executable `tailscale` in an absolute `PATH` entry; else, on macOS,
  `/Applications/Tailscale.app/Contents/MacOS/Tailscale`, the app's binary, which acts as the CLI
  when given arguments (the Mac app doesn't put `tailscale` on `PATH`). On Linux the packages put
  it on `PATH`.
- **The status is another program's output.** It is read with stdin and stderr closed, no
  descriptor of ours (`stdio_only`), at most 8 MiB and within 3 s on a thread that is abandoned at
  the deadline (a child of the client holding the pipe can't hang leon), then the client is
  killed. It is parsed as plain JSON with every field optional: a field of the wrong type counts as
  missing, at most 4096 peers are kept, and every string shown goes through `shown_text`. A peer's
  name becomes a host only through `check_label` and the `Endpoint` parser, the same checks a typed
  host gets, so a name with spaces, `;` or a leading `-` is refused. Not running (`BackendState`
  other than `Running`), not installed, or failing is one line, and nothing else changes.
- **Tailscale SSH.** A peer runs it when the status lists its `sshHostKeys` (Tailscale fills that
  only then); it answers on port 22 with the tailnet identity, so the existing ssh path connects
  without keys. For a peer without it, leon says to run `tailscale set --ssh` there or set up an ssh
  key, at once, instead of failing later with ssh's error. The first connection still needs the
  host key accepted (the app's ssh never prompts), so leon says to run `leon hosts check LABEL`
  once; a policy that asks for a browser check prints "To authenticate, visit …", which the app
  sorts as needing a login. Seeding `known_hosts` from `sshHostKeys` would skip that step, but it
  trusts the status with host keys, so it is not done.
- **leon on the other side.** A host without `leon` exits 127 from the fixed remote command, and
  the link says how to install it there (Homebrew, or a release tarball) instead of ssh's words.
- **Nothing listens.** This is still ssh: a server listening on the tailnet would be leon's first
  network listener, and is a separate decision.
