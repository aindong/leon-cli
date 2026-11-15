# Architecture: state, packaging and hosts

Persistence, scrollback, resume, terminal reads, packaging, and how the docs are tested; other
machines are in [Hosts](hosts.md). Part of [the architecture](../architecture.md).

## Decisions

- **Persistence.** The server saves its sessions to `$LEON_STATE_DIR/sessions.json` (default
  `~/.local/state/leon`) on every exit through the socket loop (`server stop`, SIGTERM, SIGINT,
  SIGHUP, idle exit), and restores them after binding (`--restore`, default true; `server stop
  --discard` skips the save and deletes the file). It also saves while it runs: the loop's 200 ms
  tick saves at most a second after the first unsaved change to the model (a session's revision,
  or the set of sessions) and checks the screens' text once a minute, writing only when the
  sessions differ from the last write (the snapshot is taken under the state lock, serialized and
  written outside it). So a crash (SIGKILL, a panic, power loss) loses a second of layout and up
  to a minute of text, and never brings back the save from the stop before it. The tick is part
  of the loop, so the final save, or `--discard`'s delete, is always the last write. Saved: sessions, windows, layout trees (split IDs and
  ratios), labels, focus, zoom, the active tab, revisions, and per block its label, keep-open, cwd,
  command, scrollback and resume command. IDs come back unchanged. Programs don't survive: a
  restored block runs a fresh login shell in its cwd (the shell process's cwd, else the last OSC 7
  path, else where it started); the saved command is never re-run, since repeating `make deploy` on
  a restart would be a surprise and would make the file a place that runs code. The dir must be private (0700, ours) and not
  flocked by another server, or nothing is saved (`server status` shows `state_file: null`); the file is 0600, written via a temp file and
  a rename. A file that is corrupt, from a newer version, or inconsistent is moved to
  `sessions.json.bad-<secs>-<n>` with a warning in the log, never replaced. So the model never
  makes a state it can't load: at most 1024 blocks (`leon_core::MAX_BLOCKS`) and 32 nested splits
  in a window (`layout::MAX_DEPTH`), refused with 400 when a block or move would pass them. Past
  about 60 splits the file nests deeper than the 128 levels serde_json reads, and so would
  `session.view` replies.
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
  agent's tool started without a terminal; and not when one of those three has `-p`, `--print` or
  `--print=…` in its argv before any `--` (Claude Code's print mode: argv, not the program's name,
  since an npm install runs as `node …/claude`). Restored blocks follow the config's
  `leon.set("resume", …)`, read once the start-up config has loaded and before restoring (the
  restore waits at most 1 s for it, since nothing is served until the restore is done; past that
  nothing is offered, as with `"never"`): `"ask"` (default) types Ctrl-U and the shell-quoted command
  at the new prompt and shows a dim hint, so Enter runs it; `"always"` adds the Enter; `"never"`
  types nothing. Typing waits (up to 60 s, then gives up) until the shell is the PTY's foreground
  process group with canonical mode off, so neither an rc file's picker nor typeahead gets it; zsh
  and fish must also have turned bracketed paste on, which their line editors do and a key an rc
  file reads (oh-my-zsh's update question, compinit's `read -q`) doesn't. bash and sh are taken at
  canonical mode alone. dash (Debian's `sh`) has no line editor and never turns canonical mode
  off, so a dash login shell is offered nothing: the wait gives up and types nothing. A command is
  offered once: when it has been typed (under `"ask"` or `"always"`), the restored block still shows
  it (`block inspect`) but it isn't saved again, so the next restart doesn't type the same stale
  command; a program that resumes registers again (the skill hook does on every agent start), and
  that registration is offered next time. A generation count, not the command's text, says which
  registration was typed, so an agent re-registering the same command isn't lost. Under `"never"`,
  or when the wait gives up, nothing is typed and the command is kept. A restart before the user
  presses Enter loses the offer, though its text stays in the restored scrollback. With `"always"` the
  private state file is a place that runs the commands programs registered, which is why `"ask"` is
  the default.
- **Terminal reads.** A thread per terminal only reads the PTY into a bounded queue (64 reads of
  up to 16 KiB); the parser thread feeds what has queued, up to 64 KiB, to the OSC 7501 tracker and
  the emulator in one go. The macOS tty buffers ~1 KiB, so reading and parsing on one thread
  stalled the program while it parsed (docs/performance.md).
- **One server per socket.** A server holds `server.lock` beside its socket (an flock) for its
  whole life, and a starting one takes it with `LOCK_NB`. Finding it held, it waits up to 2 s for
  the lock or for the socket to answer, then exits; it never unlinks a socket while the lock is
  held, since a busy server's full backlog refuses connects on macOS just as a stale socket does.
  A held lock with no socket at all (the path was removed) lets it bind anyway, unlocked. Each
  server raises its soft open-file limit to the hard one, at most 10240: launchd starts the app,
  and so the server it spawns, with 256, and each terminal holds three descriptors.
- **Version.** `crates/leon/build.rs` sets `LEON_VERSION`: the workspace version and the commit,
  `0.1.0 (1a2b3c4d5e6f)` (`git rev-parse --short=12`, or `LEON_GIT_HASH` when the build has no
  `.git`, as in Docker from a worktree). `leon --version` prints it; `server.version` keeps the bare
  version.
- **Packaging.** `scripts/bundle.sh` wraps the one `leon` binary in `Leon.app` (bundle id
  `dev.leon.app`, the bare version from `leon --version`, the brand icon `brand/icon/Leon.icns`,
  `LICENSE` and `THIRD-PARTY-NOTICES.txt` in `Contents/Resources`, ad-hoc signed); opened from Finder it runs with no arguments, which opens the app. `scripts/install.sh`
  copies the app to `~/Applications` and links `~/.local/bin/leon` to the binary inside it. Both
  replace only what they made: an app with our bundle id, or a link whose text is our binary's path.
- **Linux packages.** `scripts/package-linux.sh [--arch arm64|amd64]` builds a release binary
  (symbols stripped) in the same pinned image and writes, to `target/linux/<arch>/` (emptied
  first), the bare `leon` (which `m16a_docker_sshd` runs behind a real sshd),
  `leon-<version>-<uname -m>-linux.tar.gz` (`leon`, `README.md`, `LICENSE`,
  `THIRD-PARTY-NOTICES.txt`, `docs/`) and `leon_<version>_<arch>.deb` (hand-built with `dpkg-deb`:
  `/usr/bin/leon`, docs and the notices in `/usr/share/doc/leon`, `Depends: libc6 (>= …)` from the newest glibc symbol the binary uses, a
  fixed placeholder maintainer). It then installs the `.deb` in a clean `debian:bookworm-slim` and
  unpacks the tarball in `ubuntu:24.04` (both pinned by digest), and fails unless each `leon
  --version` prints the version. Rust links glibc dynamically: the binary needs glibc 2.34 or newer
  (measured on arm64 and amd64; no musl build yet).
- **The login shell's PATH.** Leon.app opened from Finder starts the server with launchd's bare
  PATH, and `leon server proxy` over ssh with sshd's, so programs weren't found (commands should
  run with the login shell's env). `leon server run` first runs `$SHELL -i -l -c
  /usr/bin/env` (else the passwd entry's shell) and takes its last `PATH=` line: interactive, so a
  PATH set in `~/.zshrc` counts as in the user's tab; from the system PATH; with
  `LEON_AUTOSTART=0`, so an rc line that runs leon can't start a server whose probe runs it again;
  stdin /dev/null; in a session of its own, so `zsh -i` isn't stopped for taking a terminal, and
  killed with its group after 2 s (about 20 ms normally; an rc that `exec`s tmux gives nothing and
  the PATH stays). If every dir of the server's PATH is in the login one, the login PATH is used in
  its order; otherwise the server came from a terminal with dirs of its own, which keep their
  order, and the login PATH's other dirs follow. Only absolute dirs are kept (a relative one would
  be looked up in each program's cwd), and only PATH is taken.
- **Notices and releases.** `scripts/notices.sh` lists every crate in the binary (`cargo tree`,
  normal and build dependencies, since lua-src compiles Lua into it) with its license, then each
  distinct license file from the cargo registry once; it is generated per package, never committed.
  `scripts/release.sh` clones one commit into a temp dir, builds `Leon-<version>-macos-<arch>.zip`
  and both Linux arches' packages from it, and writes `SHA256SUMS`; it never tags, pushes or uploads.
  A pushed tag runs the same scripts on GitHub, signs every file with minisign against the keys
  pinned in `crates/leon/release-keys.pub`, and publishes the release with its manifest
  (`docs/releases.md`).
- **`LEON_BIN_DIR`.** Every terminal gets the server binary's directory as `LEON_BIN_DIR` and at
  the front of `PATH` (empty entries dropped; no PATH at all falls back to the system dirs), so
  `leon` resolves inside the app. A login zsh's `path_helper` moves system dirs ahead of it, so an
  older `leon` in `/usr/local/bin` can win there; the dir is still on PATH.
- **API contract.** `help.all` and `server.version` report `api_version` (semver, `1.6.0`), apart
  from the build's version. Each method doc carries `stability` (`stable`, `experimental`,
  `internal`), and optionally `deprecated` (`since`, `replacement`, `removal`) and
  `request_example`/`response_example`; all of it comes from one table,
  `leon_server::contract`, which must name every method and block method. Stable: help, server,
  sessions, windows, targets, blocks, layout, events and the terminal's `write`, `format`, `size`,
  `resize`, `pwd`, `process`, `program_status`, `set_resume`. Experimental: the screen transport
  (`block.watch`, `screen`, `lines`, `find`, `selection`, `link_at`), `client.action`,
  `client.key`; internal: `client.keymap`. `docs/api.md` is rendered from `help.all` and
  `docs/api-snapshot.json` holds the stable schemas (prose dropped, refs inlined); tests fail when
  either drifts, and the snapshot can only be re-blessed (`LEON_BLESS=1`) with a new
  `API_VERSION`. The policy is in docs/integrators.md.
- **Themes and settings (M22a).** The theme model is `leon_core::theme` (the app re-exports it):
  `DARK` is the app's colours, `LIGHT` Iron on Parchment, and `theme::import` reads Ghostty theme
  files (also the format leon stores themes in, with its own chrome keys) and iTerm2 XML plists.
  An import keeps only the terminal palette and mixes the chrome from its background and
  foreground. Settings live only in init.lua: `leon_lua::Settings` is typed with defaults and
  records each key's `file:line`, `leon_lua::rewrite` edits one literal in a `leon.set` call and
  proves the edit by loading it before renaming it over the file, and the server's
  `server.settings` and `settings_changed` carry names, not colours; M22b resolves the names.
- **Themes and settings in the app (M22b).** The app asks `server.settings` whenever the keymap
  generation moves (every reload moves it, and `settings_changed` only comes with one), drops a
  reply asked for under an older generation, and resolves the theme names itself from the
  `themes/` beside the config the server loaded; a name that no longer loads falls back to the
  built-in theme for that appearance. Pane colours are worked out on the server, so each
  connection sends its palette (`palette` on `block.watch` and `terminal.lines`: fg, bg, cursor,
  ANSI 0-15) and its screens come resolved in it, remote hosts' too; a server older than M22b
  refuses the field and its panes keep leon-dark's colours. The Settings window (⌘,) reads
  init.lua itself and writes through `leon_lua::rewrite::write_setting`, then reloads the server,
  so init.lua stays the one place settings live.
- **Docs are tested.** `docs/cli.md` is checked against every `--help`; the `sh` blocks in the docs
  run against the real binary (a bad command line fails); `docs/lua.md`'s examples go through
  `leon config check` and `leon do -e`.
- **Naming.** Method, action, event and flag names stay as `docs/api.md` lists them, so scripts and
  agents keep working (`docs/integrators.md#compatibility`). Env vars are `LEON_*`.

- **Saved overlays (M20a).** State file version 3 adds per-window `floating` and `docked`
  arrays and permits a null tiled layout. Versions 1 and 2 still load with empty overlays.
  Save/restore preserves bounds, order, side, size and hidden flags. Validation checks
  finite contained bounds, dock sizes/sides, unique placement and visible focus before
  starting shells. Restart uses the existing fresh-shell/resume policy for hidden and
  visible blocks alike; hiding during a live session leaves programs running.
  `scripts/test-peer-uid.sh` uses the Linux test image to verify the peer uid check with
  socket permissions opened deliberately, then disables the check in a disposable copy
  and requires the rejection test to fail. The real source and server policy stay intact.
- **Saved titles.** State file version 4 adds each block's `title` (the last one its program
  set). A restored terminal starts with it, so a tab keeps its name after a restart until its
  program sets a new one; the title is cleaned again on load, since it goes on screen. Versions 1
  to 3 still load without titles; an older leon refuses version 4 as "saved by a newer leon" and
  moves the file aside.
