# Architecture: state, packaging and hosts

Persistence, scrollback, resume, terminal reads, packaging, other machines, and how the docs are tested. Part of [the architecture](../architecture.md).

## Decisions

- **Persistence.** The server saves its sessions to `$LEON_STATE_DIR/sessions.json` (default
  `~/.local/state/leon`) on every exit through the socket loop (`server stop`, SIGTERM, SIGINT,
  SIGHUP, idle exit) and restores them after binding (`--restore`, default true; `server stop --discard`
  skips the save and deletes the file). Saved: sessions, windows, layout trees (split IDs and
  ratios), labels, focus, zoom, the active tab, revisions, and per block its label, keep-open, cwd,
  command, scrollback and resume command. IDs come back unchanged. Programs don't survive: a
  restored block runs a fresh login shell in its cwd (the shell process's cwd, else the last OSC 7
  path, else where it started); the saved command is never re-run, since repeating `make deploy` on
  a restart would be a surprise and would make the file a place that runs code. The dir must be private (0700, ours) and not
  flocked by another server, or nothing is saved (`server status` shows `state_file: null`); the file is 0600, written via a temp file and
  a rename. A file that is corrupt, from a newer version, or inconsistent is moved to
  `sessions.json.bad-<secs>-<n>` with a warning in the log, never replaced.
  The app takes no part: the server it starts restores at start, before it accepts the app's
  connection, so the app's "open a session when there is none" sees the restored ones. Tests and
  benches use `embedded::start_at`, an in-process server that keeps nothing.
- **Scrollback on restore.** Each block's scrollback and screen (`capture`'s text; for an alt-screen
  app, its screen) is saved as text, capped at 2 MiB per block and 8 MiB over all blocks (each gets
  `min(2 MiB, 8 MiB / blocks)`), newest whole lines kept. Every control character but `\n` (C0, DEL,
  C1) is removed when it is saved and again when it is restored, since the file is outside the
  server's hands in between; so a snapshot can't send an escape, a query or a carriage return. The
  new terminal's emulator is fed the text, then a dim `— restored from <time> —` line, before the
  shell's first output: the text is shown, never sent to the shell. Whatever a terminal showed (a
  printed token too) is on disk in the private 0600 file until the next save replaces it;
  `server stop --discard` forgets it.
- **Resume commands.** A program registers the argv that resumes it with
  `dev.leon.terminal.set_resume {command}` (1 to 64 args, at most 8 KiB of printable ASCII, no
  backslash; null clears it). Under "ask" the user agrees to what they see typed, so it must be what
  runs: no control key, no bidi or invisible character, no byte readline could read as Meta, no
  backslash (fish reads `\'` inside single quotes). The skill hook records `claude --resume
  <session_id>` or `codex resume <session_id>` from the agent's SessionStart JSON (an id of
  `[A-Za-z0-9._-]`, no leading `-`), and only when the hook, its parent or its grandparent is its
  terminal's foreground process group: the interactive agent, not a nested `claude -p` that an
  agent's tool started without a terminal. Restored blocks follow the config's
  `leon.set("resume", …)`, read once the start-up config has loaded and before restoring (the
  restore waits at most 1 s for it, since nothing is served until the restore is done; past that
  nothing is offered, as with `"never"`): `"ask"` (default) types Ctrl-U and the shell-quoted command
  at the new prompt and shows a dim hint, so Enter runs it; `"always"` adds the Enter; `"never"`
  types nothing. Typing waits (up to 60 s, then gives up) until the shell is the PTY's foreground
  process group with canonical mode off, so neither an rc file's picker nor typeahead gets it; zsh
  and fish must also have turned bracketed paste on, which their line editors do and a key an rc
  file reads (oh-my-zsh's update question, compinit's `read -q`) doesn't. bash and sh are taken at
  canonical mode alone. dash (Debian's `sh`) has no line editor and never turns canonical mode
  off, so a dash login shell is offered nothing: the wait gives up and types nothing. The command stays registered until replaced or cleared; with `"always"` the
  private state file is a place that runs the commands programs registered, which is why `"ask"` is
  the default.
- **Terminal reads.** A thread per terminal only reads the PTY into a bounded queue (64 reads of
  up to 16 KiB); the parser thread feeds what has queued, up to 64 KiB, to the OSC 7501 tracker and
  the emulator in one go. The macOS tty buffers ~1 KiB, so reading and parsing on one thread
  stalled the program while it parsed (docs/performance.md).
- **Packaging.** `scripts/bundle.sh` wraps the one `leon` binary in `Leon.app` (bundle id
  `dev.leon.app`, version from `leon --version`, the brand icon `brand/icon/Leon.icns`, ad-hoc
  signed); opened from Finder it runs with no arguments, which opens the app. `scripts/install.sh`
  copies the app to `~/Applications` and links `~/.local/bin/leon` to the binary inside it. Both
  replace only what they made: an app with our bundle id, or a link whose text is our binary's path.
- **Linux packages.** `scripts/package-linux.sh [--arch arm64|amd64]` builds a release binary
  (symbols stripped) in the same pinned image and writes, to `target/linux/<arch>/` (emptied
  first), the bare `leon` (which `m16a_docker_sshd` runs behind a real sshd),
  `leon-<version>-<uname -m>-linux.tar.gz` (`leon`, `README.md`, `LICENSE`, `docs/`) and
  `leon_<version>_<arch>.deb` (hand-built with `dpkg-deb`: `/usr/bin/leon`, docs in
  `/usr/share/doc/leon`, `Depends: libc6 (>= …)` from the newest glibc symbol the binary uses, a
  fixed placeholder maintainer). It then installs the `.deb` in a clean `debian:bookworm-slim` and
  unpacks the tarball in `ubuntu:24.04` (both pinned by digest), and fails unless each `leon
  --version` prints the version. Rust links glibc dynamically: the binary needs glibc 2.34 or newer
  (measured on arm64 and amd64; no musl build yet).
- **`LEON_BIN_DIR`.** Every terminal gets the server binary's directory as `LEON_BIN_DIR` and at
  the front of `PATH` (empty entries dropped; no PATH at all falls back to the system dirs), so
  `leon` resolves inside the app. A login zsh's `path_helper` moves system dirs ahead of it, so an
  older `leon` in `/usr/local/bin` can win there; the dir is still on PATH.
- **Remote hosts.** `-H HOST` (a label from `leon hosts`, or `ssh://[USER@]HOST[:PORT]`) opens a
  `Link` (`leon-hosts`): a Unix socket in a fresh private dir, `link-XXXXXX/s` under the runtime
  dir (0700, socket 0600), served by threads in the client process. Each connection to it spawns
  `$LEON_SSH` (default `ssh`) as argv, never through a local shell:
  `-T -a -x -o ClearAllForwardings=yes -o ServerAliveInterval=15 -o ServerAliveCountMax=3 [-p PORT]
  -- [USER@]HOST CMD`, and copies bytes both ways, only for a peer of our uid (as the server checks). So every client that takes a socket path works unchanged over ssh: the CLI,
  `leon do` scripts, and `leon_app::remote::Remote` (`leon attach`, and the app in M16b). leon adds
  only `-T`, no forwarding (no agent on the host, no port a config's `LocalForward` would bind) and
  the keepalives (a dead Wi-Fi link is noticed in about 45 s); everything else comes from
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
  ignored. A link flocks `lock` in its dir while it lives; a killed client leaves its dir, and the
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
  restarted), it attaches again and prints a new snapshot line. `leon attach -H` uses
  `Remote::connect_retrying`: the reader reopens the fetch, call and event connections with the
  same backoff, refetches the model and re-watches the screens, since its model is state, not a
  log. Keys typed while the link is down are lost; while attached, stderr is /dev/null, so ssh's
  errors during a reconnect aren't shown.
- **Docs are tested.** `docs/cli.md` is checked against every `--help`; the `sh` blocks in the docs
  run against the real binary (a bad command line fails); `docs/lua.md`'s examples go through
  `leon config check` and `leon do -e`.
- **Naming.** Method, action, event and flag names follow `docs/spec.md`, so scripts and agents written
  for that interface carry over. Env vars are `LEON_*`.
