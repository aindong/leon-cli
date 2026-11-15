# Updates

leon updates when you ask it to:

```sh
leon self-update --check   # is there a newer release? changes nothing
leon self-update           # download it, check it, and replace this leon
```

`leon self-update --check` exits 0 when this leon is up to date (or when nothing has been released
yet), 10 when a newer release exists, and 1 when the check itself failed, so a script can tell the
three apart. `leon self-update` exits 0 when it installed the release or had nothing to do.

It replaces what is running: the `leon` binary, or the whole `Leon.app` when leon runs from inside
one (only an app that is leon's own, by its bundle id). It takes the release file for that kind,
this system and this processor; a release without one is an error, never a reason to take another
file. From a tarball only the `leon` binary is replaced: the README, notices and docs you unpacked
beside it stay as they were, and the new ones are in the release's tarball. Releases themselves, and what is in them,
are in [releases.md](releases.md).

## What is checked

Every release is described by a signed manifest, `latest.json`. Before leon installs anything:

1. **The manifest's signature.** leon fetches `latest.json` and `latest.json.minisig` and verifies
   the [minisign](https://jedisct1.github.io/minisign/) signature against the public keys that
   were in `crates/leon/release-keys.pub` when this leon was built. Nothing is read from the
   manifest before that. No flag, variable or file changes the keys a release build trusts. (A
   debug build, which no package holds, also reads the tests' keys: don't install one.)
2. **The version.** The release must be newer than this leon, by semver order. An old release is
   still correctly signed, so this is what stops a downgrade. A pre-release (`0.2.0-rc.1`) is
   taken only by a leon that is itself a pre-release. The manifest's tag must be `v` plus its
   version, and it may list only one file for each kind, system and processor.
3. **Your system.** A release that needs a newer macOS or glibc than this machine has is refused.
4. **The file.** The download stops at the size the manifest gives. The file is taken only if
   its sha256 is the manifest's and its own minisign signature verifies as well.
5. **The archive.** It is unpacked in a private directory, `.leon-update`, beside the install.
   An entry with an absolute path, with `..`, or with a link that could leave the directory (one
   with `..` or an absolute target) refuses the whole archive.
6. **The new leon.** It is run once (`leon --version`) and must report the release's version.

Only then is the new leon renamed into place, in one step: an update that is killed at any point
leaves a leon that runs, either the old one or the new one. The next `leon self-update` clears
what an interrupted one left.

Downloads go over HTTPS with certificate checks, through the system's `curl`, which leon needs
for this and nothing else. The signatures are what make an update safe, not the connection:
`curl` follows the proxy and certificate settings in your environment (not `~/.curlrc`, which
leon tells it to skip), and a wrong answer from the network can make an update fail but cannot
make leon install something else.

## Homebrew and `.deb` installs

A leon that a package manager installed is that package manager's to update. `leon self-update`
still checks for a release, then prints the command and changes nothing:

- Homebrew formula: `brew upgrade aindong/leon/leon`
- Homebrew cask (Leon.app): `brew upgrade --cask aindong/leon/leon`
- A `.deb`: the address of the new `.deb` for your processor, to download and install with
  `dpkg -i` as root.

leon tells by where it is: under Homebrew's `Cellar` or `Caskroom` (or in an app that a
`Caskroom` entry points at), or in a file that `dpkg -S` says a package owns. A leon from a
tarball, from `scripts/install.sh`, or a Leon.app you unzipped yourself updates itself.

## What leon never does

- It never updates or checks on its own. The CLI talks to the release server only when you run
  `leon self-update`.
- No telemetry. A check is one plain request for the manifest and one for its signature, plus
  the redirects the release server answers with. Each carries a fixed `User-Agent`,
  `leon/<version>`, and nothing about you or your machine: no identifier, no login, no hostname.
- It never asks for a password and never runs `sudo`. If it cannot write where leon is installed,
  it says so and stops before downloading.
- It never replaces a leon that Homebrew or dpkg installed.
- It never restarts your server or closes a session.

## A running server

Replacing the binary does not touch a server that is running: it keeps the old version, with your
sessions and the programs in them, until it restarts. To move to the new version:

```sh
leon server stop
```

That saves your sessions and ends the programs in them; the next `leon` command starts the new
server and restores the sessions. Until then the new CLI talks to the old server, which the
[API's compatibility promise](integrators.md#compatibility) covers. After updating Leon.app, quit
it and open it again.

## Another place for releases

`LEON_RELEASE_URL` points leon at another directory of release files, for a mirror or a test:

```sh
LEON_RELEASE_URL=https://mirror.example/leon leon self-update --check
```

leon then fetches `latest.json`, its signature and the release file from that directory, by name.
This is safe to set, and safe for someone else to set for you, because every signature check
above still runs against the keys built into leon: a mirror can serve only what the release key
signed, and never an older release than you have. What a mirror can do is hold you back: serve a
release that is newer than yours but not the newest, or answer as if there were none. The
address must be `https://`; plain `http://` is accepted only for a loopback address such as
`127.0.0.1`, which is what leon's own tests use. The same rule holds for every redirect, and a
server elsewhere may not redirect leon to a loopback address.

## When it refuses

Each refusal is one line, and nothing has been changed when you see it.

- **"latest.json does not match its signature"**, **"has no signature"**, or
  **"is signed by a key this leon does not trust"**: the manifest is not one leon's release key
  signed. Try again later; if it stays, something between you and the release server is changing
  the answer. A leon that is older than a [key rotation](releases.md#key-rotation) also sees the
  last of the three, and has to be updated by hand.
- **"built with no release key"**: this build predates the first release key and cannot verify
  anything. Update by hand.
- **"this leon is too old to read this release"**: the manifest's format has changed. Update by
  hand.
- **"its sha256 is not the one in the manifest"**, **"does not match its signature"** (for a
  release file), **"is larger than"**, **"ended early"**: the download was damaged or is not the
  release's file. Run it again.
- **"needs macOS … or newer"**, **"needs glibc … or newer"**: the release does not run on this
  system. Your leon keeps working.
- **"has no … file for …"**: the release has no file for this kind of install on this machine.
- **"refusing the entry …"**, **"holds no leon"**, **"does not run as leon …"**: the release's
  file is signed but not what a release should hold. Nothing was installed; please report it.
- **"is not a Leon.app"**: this leon sits inside another app's bundle, which leon will not replace.
- **"refusing …: releases come over https"**, **"refusing a redirect"**: an address (yours in
  `LEON_RELEASE_URL`, the manifest's, or a redirect's) is not one leon may fetch from.
- **"the release server answered …"**, **"too many redirects"**: the server's fault; try later.
- **"can't write to …"**: leon is installed where you cannot write. Run `leon self-update` as a
  user who can, or update by hand.
- **"another update is running in …"**: a second `leon self-update` for the same install; let the
  first finish. If none is running and **"… is in the way"** shows instead, something other than
  leon made `.leon-update` there: move it away.
- **"can't replace …"** on macOS, with "Operation not permitted": the system's App Management
  setting keeps your terminal from changing apps in `/Applications`. Allow it under System
  Settings, Privacy & Security, App Management, or update by hand.
- **"could not find the host"**, **"could not connect"**, **"timed out"**: the network. **"the
  certificate … does not check out"**: the server's certificate failed `curl`'s check.
- **"updating needs curl"**: install `curl`, or update by hand.

To update by hand, download the file for your machine from
<https://github.com/aindong/leon-cli/releases/latest> and install it as the
[README](../README.md#install) describes; [releases.md](releases.md#checking-a-download) says how
to check its signature yourself.
