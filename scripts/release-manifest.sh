#!/usr/bin/env bash
# release-manifest.sh: sign a directory of release files and write the release's manifest.
#
#   LEON_RELEASE_KEY=… scripts/release-manifest.sh [--complete] DIR
#
# DIR holds release files of this checkout's version, as scripts/release.sh and
# scripts/package-linux.sh name them; any other file there is an error. It gains SHA256SUMS,
# latest.json (docs/releases.md#the-manifest) and a minisign signature (.minisig) beside every
# release file, SHA256SUMS and latest.json. It never uploads anything.
#
# --complete        fail unless DIR holds all eight files of a full release
# LEON_RELEASE_KEY  the minisign secret key's text, as `minisign -G -W` wrote it (the GitHub
#                   secret that scripts/release-keys.sh sets; a throwaway key for a dry run)
#
# Needs minisign and jq.
set -euo pipefail

repo=$(cd "$(dirname "$0")/.." && pwd)
. "$repo/scripts/release-files.sh"
complete=""
dir=""
while [ $# -gt 0 ]; do
  case "$1" in
    --complete) complete=1; shift ;;
    -h|--help) sed -n '2,15s/^# \{0,1\}//p' "$0"; exit 0 ;;
    -*) echo "release-manifest.sh: unknown argument: $1 (try --help)" >&2; exit 2 ;;
    *) [ -z "$dir" ] || { echo "release-manifest.sh: one directory, not also $1" >&2; exit 2; }; dir=$1; shift ;;
  esac
done
[ -n "$dir" ] && [ -d "$dir" ] || { echo "release-manifest.sh: give the directory of release files (try --help)" >&2; exit 2; }
dir=$(cd "$dir" && pwd)
need minisign jq
[ -n "${LEON_RELEASE_KEY:-}" ] || { echo "release-manifest.sh: LEON_RELEASE_KEY is not set: it holds the minisign secret key (docs/releases.md#secrets)" >&2; exit 1; }

version=$("$repo/scripts/release-version.sh")
tag="v$version"
commit=${LEON_GIT_HASH:-$(git -C "$repo" rev-parse --short=12 HEAD)}
full=$(release_files "$version")
# `known NAME` prints NAME's line of the full set, or fails
known() { printf '%s\n' "$full" | awk -v n="$1" '$1 == n { print; found = 1 } END { exit !found }'; }

# what an earlier run wrote is made again
rm -f "$dir"/*.minisig "$dir/SHA256SUMS" "$dir/latest.json"
files=()
for path in "$dir"/*; do
  name=$(basename "$path")
  known "$name" >/dev/null \
    || { echo "release-manifest.sh: $name is not a release file of leon $version" >&2; exit 1; }
  files+=("$name")
done
[ ${#files[@]} -gt 0 ] || { echo "release-manifest.sh: $dir has no release files" >&2; exit 1; }
if [ -n "$complete" ]; then
  while read -r name _; do
    [ -f "$dir/$name" ] || { echo "release-manifest.sh: --complete, but $name is missing from $dir" >&2; exit 1; }
  done <<<"$full"
fi

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
(umask 077 && printf '%s\n' "$LEON_RELEASE_KEY" > "$tmp/key")
sign() { minisign -S -W -s "$tmp/key" -m "$dir/$1" >/dev/null || { echo "release-manifest.sh: minisign could not sign $1" >&2; exit 1; }; }

# the oldest systems the files run on: macOS from the app's Info.plist, glibc from the .deb's Depends
macos="" glibc=""
for name in "${files[@]}"; do
  case "$name" in
    *macos*) macos=$(sed -n 's/.*LSMinimumSystemVersion<\/key><string>\(.*\)<\/string>.*/\1/p' "$repo/scripts/bundle.sh") ;;
    *.deb)
      d=$(mktemp -d "$tmp/deb.XXXXXX")
      (cd "$d" && ar x "$dir/$name" && tar -xf control.tar.* ./control)
      floor=$(sed -n 's/^Depends:.*libc6 (>= \([0-9.]*\)).*/\1/p' "$d/control")
      [ -n "$floor" ] || { echo "release-manifest.sh: $name's Depends names no libc6 version" >&2; exit 1; }
      glibc=$(printf '%s\n' "$glibc" "$floor" | sort -V | tail -1) ;;
  esac
done

(cd "$dir" && sha256 "${files[@]}" > SHA256SUMS)
for name in "${files[@]}"; do
  sign "$name"
  read -r _ kind os arch <<<"$(known "$name")"
  jq -n --arg name "$name" --arg kind "$kind" --arg os "$os" --arg arch "$arch" \
    --arg url "$RELEASE_REPO_URL/releases/download/$tag/$name" \
    --argjson size "$(wc -c < "$dir/$name" | tr -d ' ')" \
    --arg sha256 "$(awk -v n="$name" '$2 == n { print $1 }' "$dir/SHA256SUMS")" \
    --rawfile signature "$dir/$name.minisig" \
    '{name: $name, kind: $kind, os: $os, arch: $arch, url: $url, size: $size, sha256: $sha256, signature: $signature}'
done | jq -s --arg version "$version" --arg tag "$tag" --arg commit "$commit" \
  --arg date "$(date -u +%Y-%m-%dT%H:%M:%SZ)" --arg notes "$RELEASE_REPO_URL/releases/tag/$tag" \
  --arg macos "$macos" --arg glibc "$glibc" \
  '{schema: 1, version: $version, tag: $tag, commit: $commit, date: $date, notes_url: $notes,
    min_os: ({macos: $macos, linux_glibc: $glibc} | with_entries(select(.value != ""))), files: .}' > "$dir/latest.json"
sign SHA256SUMS
sign latest.json
echo "release-manifest.sh: leon $version ($commit): ${#files[@]} release files, SHA256SUMS and latest.json are signed in $dir"
