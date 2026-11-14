#!/usr/bin/env bash
# install.sh: install Leon.app to ~/Applications and the leon CLI as a symlink into it.
#
#   scripts/install.sh [--prefix DIR] [--app PATH] [--uninstall]
#
# --prefix DIR  the CLI goes to DIR/bin/leon [default: ~/.local]
# --app PATH    the Leon.app to install [default: build target/release/Leon.app with bundle.sh]
# --uninstall   remove what an install made (use the same --prefix)
#
# Safe to run again. Needs no root, and never replaces or removes anything that isn't a leon
# install: a Leon.app with bundle id dev.leon.app, or a symlink to ~/Applications/Leon.app's binary.
set -euo pipefail

repo=$(cd "$(dirname "$0")/.." && pwd)
# an empty HOME (or /) would aim everything at /Applications and /.local
: "${HOME:?install.sh needs HOME set}"
while [ "${HOME%/}" != "$HOME" ]; do HOME=${HOME%/}; done # one spelling, so our link's text matches
[ -n "$HOME" ] || { echo "install.sh: HOME is /; set HOME to your home directory" >&2; exit 1; }
[ "$(id -u)" != 0 ] || { echo "install.sh: don't run this as root; it installs for your own user" >&2; exit 1; }
prefix="$HOME/.local"
src=""
uninstall=0
while [ $# -gt 0 ]; do
  case "$1" in
    --prefix) prefix=${2:?--prefix needs a directory}; shift 2 ;;
    --app) src=${2:?--app needs a path}; shift 2 ;;
    --uninstall) uninstall=1; shift ;;
    -h|--help) sed -n '2,12s/^# \{0,1\}//p' "$0"; exit 0 ;;
    *) echo "install.sh: unknown argument: $1 (try --help)" >&2; exit 2 ;;
  esac
done

dest="$HOME/Applications/Leon.app"
cli="$prefix/bin/leon"
target="$dest/Contents/MacOS/leon"

bundle_id() { plutil -extract CFBundleIdentifier raw -o - "$1/Contents/Info.plist" 2>/dev/null || true; }
is_leon_app() { [ ! -L "$1" ] && [ -d "$1" ] && [ "$(bundle_id "$1")" = dev.leon.app ]; }
# the link text, not where it resolves: a link left dangling after the app was trashed is still ours
is_our_link() { [ -L "$cli" ] && [ "$(readlink "$cli")" = "$target" ]; }
exists() { [ -e "$1" ] || [ -L "$1" ]; }

foreign=0
refuse() {
  foreign=1
  if [ $uninstall = 1 ]; then
    echo "install.sh: left $1: it is not a leon install" >&2
  else
    echo "install.sh: $1 exists and is not a leon install; move it away and run again" >&2
  fi
}
if exists "$dest" && ! is_leon_app "$dest"; then refuse "$dest"; fi
if exists "$cli" && ! is_our_link; then refuse "$cli"; fi

if [ $uninstall = 1 ]; then
  did=0
  if is_our_link; then rm "$cli"; echo "removed $cli"; did=1; fi
  if is_leon_app "$dest"; then rm -rf "$dest"; echo "removed $dest"; did=1; fi
  [ $foreign = 0 ] || exit 1
  [ $did = 1 ] || echo "nothing to remove"
  exit 0
fi

[ $foreign = 0 ] || exit 1

if [ -z "$src" ]; then
  src="$repo/target/release/Leon.app"
  "$repo/scripts/bundle.sh"
fi
if ! is_leon_app "$src"; then
  echo "install.sh: $src is not a Leon.app (bundle id dev.leon.app)" >&2
  exit 1
fi

mkdir -p "$HOME/Applications"
if is_leon_app "$dest" && diff -rq "$src" "$dest" > /dev/null 2>&1; then
  echo "$dest is up to date"
else
  # copy beside it, then swap, so a failed copy never leaves half an app
  stage=$(mktemp -d "$HOME/Applications/.Leon.app.XXXXXX")
  trap 'rm -rf "$stage"' EXIT
  ditto "$src" "$stage/Leon.app"
  verb=installed
  if exists "$dest"; then
    # check again: something may have appeared there while we copied
    is_leon_app "$dest" || { refuse "$dest"; exit 1; }
    rm -rf "$dest"
    verb=updated
  fi
  mv "$stage/Leon.app" "$dest"
  echo "$verb $dest ($("$target" --version))"
fi

mkdir -p "$prefix/bin"
if is_our_link; then
  echo "$cli already links to $target"
else
  ln -s "$target" "$cli"
  echo "linked $cli -> $target"
fi
# shellcheck disable=SC2016 # $PATH stays literal in the hint: the user pastes that line
case ":$PATH:" in
  *":$prefix/bin:"*) ;;
  *) printf 'note: %s is not on your PATH; add this to your shell profile:\n  export PATH=%q:"$PATH"\n' "$prefix/bin" "$prefix/bin" ;;
esac
