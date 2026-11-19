# leon

English · [简体中文](docs/i18n/zh-CN/README.md) · [日本語](docs/i18n/ja/README.md) · [한국어](docs/i18n/ko/README.md) · [Español](docs/i18n/es/README.md) · [Português (Brasil)](docs/i18n/pt-BR/README.md)

<p align="center"><img src="docs/images/banner.png" alt="leon: keep your work in view. A lion app icon beside a sketch of panes with an agent working, tests done and a review blocked." width="100%"></p>

**leon is a terminal multiplexer for people who run several terminal programs and coding agents at
once.** A server keeps your sessions, tabs and split panes, and the programs in them, running. You
work with them from a native macOS app, from the `leon` command in any terminal, from Lua scripts,
or through your coding agent, which can open a pane beside itself, run your tests there and read
the result. Programs can report whether they are working, blocked or done, so you can see which
one needs you. It runs on macOS and Linux (the app is macOS only for now) and is written in Rust.

![The leon app: an agent starting two agents and hearing back from them, the link card, renaming and closing tabs and sessions, and a cursor trail](docs/images/demo-app.gif)

![The leon command: a new session, a run that waits, typing into Python and waiting for its answer, then leon attach and a split](docs/images/demo-cli.gif)

## Features

- **Persistent layout.** Programs keep running when the app quits: closing the window or ⌘Q only
  detaches, and the app opens again on the same sessions, tabs, splits and live screens. When the
  server itself stops, sessions, tabs, splits and each pane's directory are saved and come back on
  its next start, with the same IDs you script against (the programs in them don't).
- **The `leon` command.** `leon run`, `send`, `capture` and `wait` open panes, type into
  them, read their screens and wait for programs, with fixed exit codes and `--json` output.
  `leon attach` shows a session in any terminal, over ssh too.
- **The macOS app.** Sidebar or titlebar tabs, splits, floating panes, a scratch drawer, a
  command palette, a session switcher, scrollback with find and copy mode, and eight themes.
- **Agents.** `leon skill setup` teaches Claude Code and Codex to work in panes beside
  themselves, and their panes offer to resume the same conversation after a restart.
- **Status and attention.** A program that writes an OSC 7501 report (agents do) shows its state
  on its tab and pane. The Activity list, the Dock badge and notifications tell you what is
  blocked, failed or finished.
- **Remote hosts.** `-H devbox` acts on the leon server on another machine through your own `ssh`,
  and the app shows each host's sessions beside your own.
- **Lua.** Key bindings, modes, your own actions, and scripts that react to events.
- **An API.** Every client uses the same socket API, which describes itself (`leon api list`)
  and has a compatibility promise for anyone writing their own client.

## Install

### Homebrew

```sh
brew install --cask aindong/leon/leon   # macOS: Leon.app, with the leon command
brew install aindong/leon/leon          # the CLI alone, on macOS or Linux
```

Pick one: the cask's `leon` is the one inside the app. `brew upgrade` brings new releases.
Leon.app is not notarized by Apple, so macOS will not open it while it carries the quarantine flag
that Homebrew puts on a download. After the cask installs, and before you open the app, remove it:

```sh
xattr -dr com.apple.quarantine /Applications/Leon.app
```

### From a release

Each release is at <https://github.com/aindong/leon-cli/releases/latest>: `Leon-<version>-macos-<arch>.zip`
(Leon.app), a tarball of the CLI for macOS and for Linux, and a `.deb`, each for arm64 and x86_64.
On macOS, unzip the app into `/Applications` and run the `xattr` command above (a file from a
browser is quarantined too). Opened before that, macOS refuses it (it says it could not verify
Leon is free of malware); run the command and open it again. The CLI tarball for macOS carries
the flag onto its `leon` as well: after unpacking, run `xattr -dr com.apple.quarantine` on its
folder. On Linux, with the file for your machine downloaded:

```sh
dpkg -i leon_*.deb                               # Debian or Ubuntu, as root
mkdir -p ~/.local/bin                            # anywhere else: make the folder,
tar xzf leon-*-linux.tar.gz -C ~/.local          # unpack it,
ln -s ~/.local/leon-*-linux/leon ~/.local/bin/   # and put its leon on your PATH
```

Every file is signed: [docs/releases.md](docs/releases.md#checking-a-download) says how to check
one.

### Updating

```sh
leon self-update --check   # is there a newer release?
leon self-update           # download it, check its signature, and replace this leon
```

leon updates only when you ask, and checks the release's signature before it installs anything.
A leon from Homebrew is updated by `brew upgrade`, and one from a `.deb` by installing the new
`.deb`; `leon self-update` tells you which. A running server keeps the old version until
`leon server stop`. The details are in [docs/updates.md](docs/updates.md).

### From source

To build leon yourself, `scripts/install.sh` on macOS or `cargo` on Linux:
[docs/install.md](docs/install.md) has the steps, and the files a release has.

## The first five commands

A two-minute quickstart, from any terminal (the server starts by itself). Python stands in for any
program you want to drive:

```sh
leon new demo                                    # 1. a session called "demo"
leon -s demo run --label py -- python3 -q        # 2. run a program in a new tab
leon -s demo wait -b py --text '>>>'             #    and wait for its prompt
leon -s demo send -b py 'print(6 * 7)'           # 3. type into it (no Enter yet)
leon -s demo send-key -b py enter                # 4. press Enter
leon -s demo wait -b py --text 42                #    and wait for the answer
leon -s demo capture -b py --trim                # 5. read its screen: 42
```

Targets (`-s` session, `-b` block, leon's name for a pane) are an ID, a label, a position or the
end of an ID. Inside a leon terminal you can drop `-s`: commands act on your own session. `leon ls`
lists sessions; `leon --help` lists everything, with the exit codes at the bottom.

## Split it

```sh
leon -s demo split -b py --split below --ratio 30 -- top     # a pane under py, 30% high
leon -s demo focus up
leon -s demo zoom                                            # fill the window; again to undo
```

In the app: ⌘D splits right, ⌘⇧D splits down, ⌘⌥ and an arrow moves between panes, and dragging a
divider resizes. `leon attach demo` shows the session in this terminal (ctrl+b d detaches). When
you are done, `leon kill demo` ends the session and its programs.

## In any terminal: `leon attach`

```text
leon attach                  # pick a session, tab or block (type to filter), Enter attaches
leon attach demo             # straight to the session "demo"
ssh -t box leon attach       # the same, on another machine's leon
```

`leon attach` shows the tab's split panes and a status bar, and sends your keys to the focused
pane. After the prefix, ctrl+b (`--prefix` or the config's `leon.mode("attach", { prefix })` to
change it):

| Keys | Action |
|---|---|
| ctrl+b d | Detach; every program keeps running |
| ctrl+b ←↑↓→, ctrl+b h/j/k/l | Focus the pane that way |
| ctrl+b %, ctrl+b " | Split right, split down |
| ctrl+b z | Zoom the pane (again to undo) |
| ctrl+b c | New tab |
| ctrl+b n, ctrl+b p | Next, previous tab |
| ctrl+b s | Back to the picker |

Your `init.lua` bindings work there too, except `cmd` keys (a terminal never sends them) and a
binding on the prefix key itself.

## Work on your devbox

Commands that talk to the server take `-H` to act on the leon server on another machine instead,
through your own `ssh` (no account, relay or VPN; `~/.ssh/config` and `ProxyJump` work). Put `leon`
on the other machine's PATH or in `~/.local/bin`; the first command there starts its server, and
programs keep running after you disconnect.

```text
leon hosts add devbox ssh://me@devbox.example.com
leon -H devbox run -c /home/me/app -- make test   # runs on the devbox, in its /home/me/app
leon -H devbox attach               # its sessions, in this terminal
leon hosts check
leon hosts add --tailscale          # from your tailnet (Tailscale SSH: no keys)
```

Dropped links, keys typed while one is down, and the hosts in the app's sidebar are in
[docs/remote-hosts.md](docs/remote-hosts.md).


## Let an agent drive it

```sh
leon skill setup
```

installs a skill and hooks for Claude Code and Codex (each one whose config dir exists or whose
program is on your PATH; it says which, and why). The hooks show each agent's state on its pane:
working, waiting on you, or done. Then, working in a leon terminal, ask "run the tests beside
you", and the agent does what you would:

```sh
id=$(leon split --label tests --keep-open -- cargo test)
leon wait "$id"
leon capture -b "$id" --trim | tail -20
leon block close "$id"
```

It knows where it is from `LEON_SESSION` and `LEON_BLOCK`, labels what it opens, and closes only
what it created. An agent can also start other agents (`leon agent start claude "the task"`) and
hear back from them: their reports go to its `leon inbox`, and leon types a short notice into its
prompt when its turn has ended. Permission prompts stay yours: no agent in leon can type into a
pane that waits on one. See [docs/agents.md](docs/agents.md).

## Know what needs you

Programs report whether they are working, blocked or done, and the app shows it on their tabs and
panes, in the Activity list (⌘⇧A), on the Dock and in notifications.
[docs/shortcuts.md](docs/shortcuts.md#know-what-needs-you) has the details and the settings.

## App shortcuts

Every key the app has, from ⌘N to ⌘Q, with how to change one, is in
[docs/shortcuts.md](docs/shortcuts.md).


## Configuration

leon reads `~/.config/leon/init.lua` (or `$LEON_CONFIG`). Settings have their own commands, and the
app's Settings window (⌘,) writes the same file:

```sh
leon config set font.size 15
leon config set theme dark=leon-dark,light=leon-light
leon theme list
leon config check
```

The same file defines key bindings, modes and your own actions, and `leon do` runs scripts:

```lua
leon.action{
  name = "name_session",
  title = "Name this session",
  args = { label = "string" },
  run = function(ctx, args) return leon.session.set_label{ label = args.label } end,
}
```

```sh
leon config reload
leon do name_session label=api
leon do -e 'print(#leon.session.view().windows .. " tabs")'
```

`leon theme import FILE` brings in another terminal's theme. See [docs/lua.md](docs/lua.md) and
[docs/cli/settings.md](docs/cli/settings.md).

## Roadmap

leon 0.1.0 is the first release. What has landed, milestone by milestone, is in
[docs/roadmap.md](docs/roadmap.md) and [CHANGELOG.md](CHANGELOG.md). What comes next:

- **The app on Linux.** Linux has the server, the CLI, `leon attach`, agents and Lua; the GPUI app
  is macOS only for now.
- **Two clients on one tab.** With the app and `leon attach` showing the same tab, each pane takes
  the size of the client that last changed its layout, and the other client clips or pads it.
- **Lookalike link hosts.** ⌘-click opens an `http(s)` link without asking, even when its host uses
  non-ASCII letters that look like another's; only other kinds of link show their address first.
- **`codex exec` runs.** leon tells a one-shot `claude -p` run from an interactive agent, but not
  yet a `codex exec` run, which can still be offered a resume meant for an interactive session.

## Docs

- [docs/cli.md](docs/cli.md): every command and flag.
- [docs/lua.md](docs/lua.md): the config and scripts, with examples that a test runs.
- [docs/agents.md](docs/agents.md): agents driving leon, and the OSC 7501 status format.
- [docs/recipes.md](docs/recipes.md): project and git worktree sessions as a Lua recipe.
- [docs/integrators.md](docs/integrators.md): writing a client for leon's socket API in any
  language, and the compatibility promise. [docs/api.md](docs/api.md): every method.
- [docs/architecture.md](docs/architecture.md): how it's built.
  [docs/performance.md](docs/performance.md): what it costs, measured.
- [docs/releases.md](docs/releases.md): how a release is cut and signed, and the manifest format.
- [CHANGELOG.md](CHANGELOG.md): what changed.

## Build and test

```sh
cargo build
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

GPUI is built with its `runtime_shaders` feature, so you don't need Xcode's Metal toolchain. The
terminal tests run a real shell in a PTY, and the doc tests run the commands in these docs.
`scripts/linux-test.sh` runs the tests for everything but the app on Linux, in Docker. The release
tests sign with a throwaway key, so they need `minisign` and `jq`.

## Contributing

Bug reports, ideas, code and translations are welcome. [CONTRIBUTING.md](CONTRIBUTING.md) has the
setup, the house rules and how a change lands; [docs/i18n/README.md](docs/i18n/README.md) says how
to translate the README and the guides. Everyone taking part follows the
[Code of Conduct](CODE_OF_CONDUCT.md), and a security problem is reported privately, as
[SECURITY.md](SECURITY.md) says.

## License

[MIT](LICENSE). The Instrument Sans font in `brand/fonts` is under the
[SIL Open Font License](brand/fonts/OFL.txt).
