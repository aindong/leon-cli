#!/usr/bin/env bash
# release-version.sh: print the version in Cargo.toml; with a tag, first check that the tag names it.
#
#   scripts/release-version.sh [--on-main] [TAG]
#
# TAG        a release tag; anything but v<version> exits 1, so a release never ships under another name
# --on-main  also exit 1 unless this checkout's commit is on origin/main: releases are cut from
#            main only. It needs the history (a checkout with fetch-depth 0), not a shallow clone.
set -euo pipefail

on_main=""
case "${1:-}" in
  -h|--help) sed -n '2,8s/^# \{0,1\}//p' "$0"; exit 0 ;;
  --on-main) on_main=1; shift ;;
esac
repo=$(cd "$(dirname "$0")/.." && pwd)
version=$(sed -n 's/^version = "\(.*\)"$/\1/p' "$repo/Cargo.toml" | head -1)
[ -n "$version" ] || { echo "release-version.sh: no version in $repo/Cargo.toml" >&2; exit 1; }
if [ $# -gt 0 ] && [ "$1" != "v$version" ]; then
  echo "release-version.sh: the tag is $1, but Cargo.toml says $version: the tag must be v$version" >&2
  exit 1
fi
if [ -n "$on_main" ]; then
  git -C "$repo" rev-parse --verify --quiet 'origin/main^{commit}' >/dev/null \
    || { echo "release-version.sh: no origin/main to compare with: fetch it (the checkout needs its full history)" >&2; exit 1; }
  if ! git -C "$repo" merge-base --is-ancestor HEAD origin/main; then
    echo "release-version.sh: this commit ($(git -C "$repo" rev-parse --short=12 HEAD)) is not on main." >&2
    echo "  A release is a commit of main: merge dev into main with a pull request, then tag main (docs/releases.md)." >&2
    exit 1
  fi
fi
echo "$version"
