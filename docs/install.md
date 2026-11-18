# Building leon from source

Part of the [README](../README.md#install).

To build leon yourself you need a Rust toolchain ([rustup](https://rustup.rs)) and git. The two
sections below build it on macOS and on Linux.

## macOS

You need macOS 11 or later. From a checkout of this repository:

```sh
scripts/install.sh
```

That builds `Leon.app`, copies it to `~/Applications`, and links the `leon` command into
`~/.local/bin` (use `--prefix DIR` for `DIR/bin`). It never uses root, prints each thing it does,
is safe to run again, and refuses to replace anything at those paths that it didn't install.
`scripts/install.sh --uninstall` removes both. Open **Leon** from Finder, Spotlight or the Dock, or
type `leon`. To build only the app, `scripts/bundle.sh` writes `target/release/Leon.app`.

The app is a client of the leon server. It attaches to the one running (from the shell or an
earlier launch) or starts it. Quitting the app leaves the server and your programs running;
`leon server stop`, or **Quit and End All Sessions…** in the leon menu, ends them.

## Linux

On Linux leon is the server, the CLI and `leon attach` (agents and Lua scripts too); the app isn't
there yet, so `leon` alone in a terminal runs `leon attach`. Build it with `cargo`:

```sh
cargo build --release -p leon
```

That gives a headless `target/release/leon`; put it on your `PATH`. It needs glibc 2.34 or newer
(Debian 12, Ubuntu 22.04, RHEL 9 and later). The socket goes in `$XDG_RUNTIME_DIR/leon` when your
session sets it. With Docker you can build packages instead:

```sh
scripts/package-linux.sh                 # or --arch amd64 / --arch arm64
```

That writes a tarball and a `.deb` to `target/linux/<arch>/` and checks that both install and run
in clean containers. Install the `.deb` on Debian or Ubuntu with `dpkg` (as root):

```sh
dpkg -i target/linux/$(dpkg --print-architecture)/leon_*.deb
```

or unpack the tarball anywhere and put its `leon` on your `PATH`:

```sh
tar xzf target/linux/*/leon-*-"$(uname -m)"-linux.tar.gz -C ~/.local
ln -s ~/.local/leon-*-linux/leon ~/.local/bin/leon
```

## Release files

`scripts/release.sh` builds, from a clean clone of one commit, the files a release has:
`Leon-<version>-macos-<arch>.zip` (Leon.app, signed ad hoc and not notarized), the CLI tarball for
macOS, a tarball and a `.deb` for Linux on arm64 and x86_64, and `SHA256SUMS`. It runs on macOS and
needs Docker for the Linux packages. `leon --version` names the release and the commit it was
built from. A pushed tag does the same on GitHub, signs every file and publishes the release:
[releases.md](releases.md).

A throwaway line for the CI proof.
