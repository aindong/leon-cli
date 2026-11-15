#!/usr/bin/env bash
# release-version.sh: print the version in Cargo.toml; with a tag, first check that the tag names it.
#
#   scripts/release-version.sh [TAG]
#
# TAG  a release tag; anything but v<version> exits 1, so a release never ships under another name
set -euo pipefail

repo=$(cd "$(dirname "$0")/.." && pwd)
version=$(sed -n 's/^version = "\(.*\)"$/\1/p' "$repo/Cargo.toml" | head -1)
[ -n "$version" ] || { echo "release-version.sh: no version in $repo/Cargo.toml" >&2; exit 1; }
if [ $# -gt 0 ] && [ "$1" != "v$version" ]; then
  echo "release-version.sh: the tag is $1, but Cargo.toml says $version: the tag must be v$version" >&2
  exit 1
fi
echo "$version"
