#!/usr/bin/env bash
# release-dry-run.sh: build a release on this Mac and prove it signs and verifies, with a key that
# is made for the run and thrown away. Nothing is tagged, pushed or uploaded.
#
#   scripts/release-dry-run.sh [--out DIR] [release.sh arguments, e.g. --linux none]
#
# --out DIR  where the files go [default: target/release-files/dry-run]
#
# Runs scripts/release.sh, signs with scripts/release-manifest.sh, checks with
# scripts/release-verify.sh, then changes one byte of a release file and of the manifest and
# expects the check to fail each time. Exits 0 only if all of that held. The files stay in DIR,
# signed by a key nobody has: they are for looking at, never for publishing.
set -euo pipefail

repo=$(cd "$(dirname "$0")/.." && pwd)
out="$repo/target/release-files/dry-run"
args=()
while [ $# -gt 0 ]; do
  case "$1" in
    --out) out=${2:?--out needs a directory}; shift 2 ;;
    -h|--help) sed -n '2,12s/^# \{0,1\}//p' "$0"; exit 0 ;;
    *) args+=("$1"); shift ;;
  esac
done
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

"$repo/scripts/release.sh" --out "$out" ${args[@]+"${args[@]}"}
minisign -G -W -p "$tmp/key.pub" -s "$tmp/key" >/dev/null
tail -1 "$tmp/key.pub" > "$tmp/keys"
LEON_RELEASE_KEY=$(cat "$tmp/key") "$repo/scripts/release-manifest.sh" "$out"
verify() { "$repo/scripts/release-verify.sh" --keys "$tmp/keys" "$1"; }
verify "$out"

# a changed byte must fail: in a release file (its signature), and in the manifest (its own)
first=$(awk 'NR == 1 { print $2 }' "$out/SHA256SUMS")
for name in "$first" latest.json; do
  rm -rf "$tmp/changed" && cp -R "$out" "$tmp/changed"
  printf 'x' | dd of="$tmp/changed/$name" bs=1 seek=7 conv=notrunc 2>/dev/null
  if verify "$tmp/changed" >/dev/null 2>&1; then
    echo "release-dry-run.sh: FAIL: a changed byte in $name still verified" >&2
    exit 1
  fi
  echo "release-dry-run.sh: ok: a changed byte in $name fails verification"
done
echo "release-dry-run.sh: ok: the release in $out builds, signs and verifies (throwaway key: don't publish these)"
