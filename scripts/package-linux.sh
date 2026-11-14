#!/usr/bin/env bash
# Linux release packages, built in the pinned Rust image (scripts/linux-image.sh): a tarball
# (leon, README.md, LICENSE, docs/) and a .deb, both in target/linux/. Then each is installed in a
# clean container and must answer `leon --version`. Needs Docker.
#   scripts/package-linux.sh [--arch arm64|amd64]
set -euo pipefail
cd "$(dirname "$0")/.."
. scripts/linux-image.sh
out="$PWD/target/linux"
mkdir -p "$out"
DOCKER_ARGS=(-v "$out":/out -e "MAINTAINER=$(git log -1 --format='%an <%ae>')")

in_image bash -c '
    set -euo pipefail
    CARGO_PROFILE_RELEASE_STRIP=symbols cargo build --locked --release -p leon
    bin=/target/release/leon
    version=$("$bin" --version | cut -d" " -f2)
    # Rust links glibc dynamically: the .deb depends on the newest symbol version the binary uses.
    glibc=$(objdump -T "$bin" | grep -o "GLIBC_[0-9.]*" | sed s/GLIBC_// | sort -V | tail -1)
    stage=$(mktemp -d)

    name="leon-$version-$(uname -m)-linux"
    mkdir "$stage/$name"
    cp "$bin" README.md LICENSE "$stage/$name/"
    cp -R docs "$stage/$name/docs"
    tar -C "$stage" -czf "/out/$name.tar.gz" --owner=0 --group=0 "$name"

    arch=$(dpkg --print-architecture)
    root="$stage/deb"
    install -D -m 755 "$bin" "$root/usr/bin/leon"
    install -d "$root/usr/share/doc/leon"
    cp -R README.md docs "$root/usr/share/doc/leon/"
    install -m 644 LICENSE "$root/usr/share/doc/leon/copyright"
    install -d "$root/DEBIAN"
    cat > "$root/DEBIAN/control" <<CONTROL
Package: leon
Version: $version
Architecture: $arch
Maintainer: $MAINTAINER
Depends: libc6 (>= $glibc)
Section: utils
Priority: optional
Description: fast, scriptable terminal multiplexer (server and CLI)
 A long-lived server owns sessions, windows, split layouts and terminal
 panes; the leon command, Lua scripts and agents drive it over one socket.
CONTROL
    chmod -R go-w "$root"
    dpkg-deb --root-owner-group --build "$root" "/out/leon_${version}_$arch.deb" >/dev/null
    echo "version=$version glibc>=$glibc"
    ls -l /out
'

# Install each package where no Rust and no leon have ever been, and ask it its version.
deb=$(ls -t "$out"/leon_*_"$ARCH".deb | head -1)
tarball=$(ls -t "$out"/leon-*-"$( [ "$ARCH" = amd64 ] && echo x86_64 || echo aarch64)"-linux.tar.gz | head -1)
want="leon $(basename "$deb" | cut -d_ -f2)"
got=$(docker run --rm --platform "$PLATFORM" -v "$out":/out:ro debian:bookworm-slim \
    sh -c "dpkg -i /out/$(basename "$deb") >/dev/null && leon --version")
[ "$got" = "$want" ] || { echo "FAIL: the .deb's leon --version printed '$got', not '$want'" >&2; exit 1; }
echo "ok: $(basename "$deb") installs on debian:bookworm-slim: $got"
got=$(docker run --rm --platform "$PLATFORM" -v "$out":/out:ro ubuntu:24.04 \
    sh -c "cd /tmp && tar xzf /out/$(basename "$tarball") && ./leon-*/leon --version && ls leon-*/LICENSE leon-*/README.md leon-*/docs/cli.md >/dev/null")
[ "$got" = "$want" ] || { echo "FAIL: the tarball's leon --version printed '$got', not '$want'" >&2; exit 1; }
echo "ok: $(basename "$tarball") runs on ubuntu:24.04: $got"
