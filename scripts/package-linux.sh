#!/usr/bin/env bash
# Linux release packages, built in the pinned Rust image (scripts/linux-image.sh): a tarball
# (leon, README.md, LICENSE, THIRD-PARTY-NOTICES.txt, docs/), a .deb and the bare binary, in target/linux/<arch>/.
# Then each is installed in a clean container and must answer `leon --version`. Needs Docker.
#   scripts/package-linux.sh [--arch arm64|amd64]
set -euo pipefail
cd "$(dirname "$0")/.."
. scripts/linux-image.sh
[ ${#ARGS[@]} -eq 0 ] || { echo "usage: scripts/package-linux.sh [--arch arm64|amd64]" >&2; exit 2; }
out="$PWD/target/linux/$ARCH"
mkdir -p "$out"
rm -f "$out"/leon "$out"/leon[-_]* # only this run's files, so the checks below can't pick up an old one
# the commit for `leon --version`, from here: a worktree's .git points outside the mounted source
hash=$(git rev-parse --short=12 HEAD 2>/dev/null || true)
DOCKER_ARGS=(-v "$out":/out -e "LEON_GIT_HASH=$hash")

in_image bash -c '
    set -euo pipefail
    CARGO_PROFILE_RELEASE_STRIP=symbols cargo build --locked --release -p leon
    bin=/target/release/leon
    version=$("$bin" --version | cut -d" " -f2)
    # Rust links glibc dynamically: the .deb depends on the newest symbol version the binary uses.
    glibc=$(objdump -T "$bin" | grep -o "GLIBC_[0-9.]*" | sed s/GLIBC_// | sort -V | tail -1)
    stage=$(mktemp -d)
    scripts/notices.sh > "$stage/THIRD-PARTY-NOTICES.txt"

    name="leon-$version-$(uname -m)-linux"
    mkdir "$stage/$name"
    cp "$bin" README.md LICENSE "$stage/THIRD-PARTY-NOTICES.txt" "$stage/$name/"
    cp -R docs "$stage/$name/docs"
    tar -C "$stage" -czf "/out/$name.tar.gz" --owner=0 --group=0 "$name"
    cp "$bin" /out/leon # the bare binary too, for tests that run it in a container (m16a_docker_sshd)

    arch=$(dpkg --print-architecture)
    root="$stage/deb"
    install -D -m 755 "$bin" "$root/usr/bin/leon"
    install -d "$root/usr/share/doc/leon"
    cp -R README.md docs "$root/usr/share/doc/leon/"
    install -m 644 LICENSE "$root/usr/share/doc/leon/copyright"
    install -m 644 "$stage/THIRD-PARTY-NOTICES.txt" "$root/usr/share/doc/leon/"
    install -d "$root/DEBIAN"
    cat > "$root/DEBIAN/control" <<CONTROL
Package: leon
Version: $version
Architecture: $arch
Maintainer: leon <noreply@leon.invalid>
Depends: libc6 (>= $glibc), libgcc-s1
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
debs=("$out"/leon_*.deb) tarballs=("$out"/leon-*-linux.tar.gz)
[ ${#debs[@]} -eq 1 ] && [ ${#tarballs[@]} -eq 1 ] || { echo "FAIL: expected one .deb and one tarball in $out" >&2; exit 1; }
deb=$(basename "${debs[0]}") tarball=$(basename "${tarballs[0]}")
want="leon $(echo "$deb" | cut -d_ -f2)${hash:+ ($hash)}"
got=$(docker run --rm --platform "$PLATFORM" -v "$out":/out:ro "$DEBIAN_IMAGE" \
    sh -c 'dpkg -i "/out/$1" >/dev/null && leon --version' sh "$deb")
[ "$got" = "$want" ] || { echo "FAIL: the .deb's leon --version printed '$got', not '$want'" >&2; exit 1; }
echo "ok: $deb installs on ${DEBIAN_IMAGE%@*}: $got"
got=$(docker run --rm --platform "$PLATFORM" -v "$out":/out:ro "$UBUNTU_IMAGE" \
    sh -c 'cd /tmp && tar xzf "/out/$1" && d=${1%.tar.gz} && "$d/leon" --version && ls "$d/LICENSE" "$d/THIRD-PARTY-NOTICES.txt" "$d/README.md" "$d/docs/cli.md" >/dev/null' sh "$tarball")
[ "$got" = "$want" ] || { echo "FAIL: the tarball's leon --version printed '$got', not '$want'" >&2; exit 1; }
echo "ok: $tarball runs on ${UBUNTU_IMAGE%@*}: $got"
