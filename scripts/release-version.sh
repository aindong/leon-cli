#!/usr/bin/env bash
# release-version.sh: print the version in Cargo.toml; with a tag, first check that the tag names it.
#
#   scripts/release-version.sh [--on-main] [TAG]
#
# TAG        a release tag; anything but v<version> exits 1, so a release never ships under another name
# --on-main  also exit 1 unless this checkout's commit is on origin/main: releases are cut from
#            main only. It needs the history (a checkout with fetch-depth 0), not a shallow clone.
set -euo pipefail

on_main="" tag=""
while [ $# -gt 0 ]; do
  case "$1" in
    -h|--help) sed -n '2,8s/^# \{0,1\}//p' "$0"; exit 0 ;;
    --on-main) on_main=1 ;;
    *) [ -z "$tag" ] || { echo "release-version.sh: one tag, not also $1 (try --help)" >&2; exit 2; }; tag=$1 ;;
  esac
  shift
done
repo=$(cd "$(dirname "$0")/.." && pwd)
version=$(sed -n 's/^version = "\(.*\)"$/\1/p' "$repo/Cargo.toml" | head -1)
[ -n "$version" ] || { echo "release-version.sh: no version in $repo/Cargo.toml" >&2; exit 1; }
if [ -n "$tag" ] && [ "$tag" != "v$version" ]; then
  echo "release-version.sh: the tag is $tag, but Cargo.toml says $version: the tag must be v$version" >&2
  exit 1
fi
if [ -n "$on_main" ]; then
  main=refs/remotes/origin/main # the full name: a tag called origin/main must not stand in for it
  git -C "$repo" rev-parse --verify --quiet "$main^{commit}" >/dev/null \
    || { echo "release-version.sh: no origin/main to compare with: fetch it (the checkout needs its full history)" >&2; exit 1; }
  if ! git -C "$repo" merge-base --is-ancestor HEAD "$main"; then
    echo "release-version.sh: this commit ($(git -C "$repo" rev-parse --short=12 HEAD)) is not on main." >&2
    echo "  A release is a commit of main: merge dev into main with a pull request, then tag main (docs/releases.md)." >&2
    exit 1
  fi
fi
echo "$version"
