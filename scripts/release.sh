#!/usr/bin/env bash
# release.sh: build leon's release files from a clean checkout of one commit, then print their
# SHA-256 sums. It never tags, pushes or uploads anything.
#
#   scripts/release.sh [--commit REV] [--linux ARCHES] [--out DIR]
#
# --commit REV     the commit to build [default: HEAD]; it is cloned into a temp dir, so
#                  uncommitted edits never ship
# --linux ARCHES   Linux packages to build: arm64, amd64, both as arm64,amd64, or none [default: arm64,amd64]
# --out DIR        where the files go [default: target/release-files/<version>]
#
# Makes Leon-<version>-macos-<arch>.zip (Leon.app, scripts/bundle.sh), per Linux arch a tarball and
# a .deb (scripts/package-linux.sh, needs Docker), and SHA256SUMS. Runs on macOS. A set
# CARGO_TARGET_DIR is used for the macOS build (its dependencies' cache); the source is still the clone.
set -euo pipefail

repo=$(cd "$(dirname "$0")/.." && pwd)
commit=HEAD
linux=arm64,amd64
out=""
while [ $# -gt 0 ]; do
  case "$1" in
    --commit) commit=${2:?--commit needs a revision}; shift 2 ;;
    --linux) linux=${2:?--linux needs arm64, amd64, arm64,amd64 or none}; shift 2 ;;
    --out) out=${2:?--out needs a directory}; shift 2 ;;
    -h|--help) sed -n '2,15s/^# \{0,1\}//p' "$0"; exit 0 ;;
    *) echo "release.sh: unknown argument: $1 (try --help)" >&2; exit 2 ;;
  esac
done
arches=()
case "$linux" in
  none) ;;
  arm64|amd64) arches=("$linux") ;;
  arm64,amd64|amd64,arm64) arches=(arm64 amd64) ;;
  *) echo "release.sh: --linux takes arm64, amd64, arm64,amd64 or none, not $linux" >&2; exit 2 ;;
esac
[ "$(uname -s)" = Darwin ] || { echo "release.sh: builds Leon.app, so it runs on macOS" >&2; exit 1; }

rev=$(git -C "$repo" rev-parse --verify --quiet "$commit^{commit}") \
  || { echo "release.sh: $commit is not a commit" >&2; exit 1; }
hash=$(git -C "$repo" rev-parse --short=12 "$rev")
if [ -n "$(git -C "$repo" status --porcelain)" ]; then
  echo "release.sh: note: this checkout has uncommitted changes; they are not in the release"
fi

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
src="$tmp/leon"
echo "release.sh: a clean checkout of $hash in $src"
git clone --quiet --no-checkout "$repo" "$src"
git -C "$src" checkout --quiet --detach "$rev"

version=$(sed -n 's/^version = "\(.*\)"$/\1/p' "$src/Cargo.toml" | head -1)
out=${out:-"$repo/target/release-files/$version"}
mkdir -p "$out"
rm -f "$out"/Leon-*.zip "$out"/leon[-_]* "$out"/SHA256SUMS # only this run's files

echo "release.sh: cargo build --locked --release -p leon"
target_dir=${CARGO_TARGET_DIR:-"$src/target"}
(cd "$src" && CARGO_PROFILE_RELEASE_STRIP=symbols cargo build --locked --release -p leon --target-dir "$target_dir")
bin="$target_dir/release/leon"
got=$(LEON_AUTOSTART=0 "$bin" --version)
[ "$got" = "leon $version ($hash)" ] || { echo "release.sh: the build says '$got', not 'leon $version ($hash)'" >&2; exit 1; }

"$src/scripts/bundle.sh" --bin "$bin" --out "$tmp/app/Leon.app"
zip="$out/Leon-$version-macos-$(uname -m).zip"
ditto -c -k --keepParent "$tmp/app/Leon.app" "$zip"
echo "release.sh: wrote $zip"

for a in ${arches[@]+"${arches[@]}"}; do
  echo "release.sh: Linux $a (scripts/package-linux.sh)"
  (cd "$src" && scripts/package-linux.sh --arch "$a")
  cp "$src/target/linux/$a"/leon-*-linux.tar.gz "$src/target/linux/$a"/leon_*.deb "$out/"
done

cd "$out"
files=(Leon-*.zip)
for f in leon[-_]*; do [ -e "$f" ] && files+=("$f"); done
shasum -a 256 "${files[@]}" > SHA256SUMS
echo "release.sh: leon $version ($hash), in $out:"
cat SHA256SUMS
