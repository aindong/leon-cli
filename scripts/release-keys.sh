#!/usr/bin/env bash
# release-keys.sh: make the key that leon's releases are signed with. Run it once, yourself.
#
#   scripts/release-keys.sh [--name SECRET] [--keys FILE] [--repo OWNER/NAME]
#
# Makes a minisign key pair in a private temp dir, stores the secret key as a GitHub Actions secret
# (`gh secret set` reads it from the file: it is never printed and never on a command line),
# deletes the temp dir, and adds the public key to the keys leon pins. Commit that file.
#
# --name SECRET      the secret's name [default: LEON_RELEASE_KEY]; a rotation uses a second name
#                    (docs/releases.md#key-rotation)
# --keys FILE        where the public keys are pinned [default: crates/leon/release-keys.pub]
# --repo OWNER/NAME  the GitHub repository [default: the one gh finds for this checkout]
#
# It never replaces a secret that exists: that key would be gone for good, and installed leons
# trust only the keys they shipped with. Needs minisign, and gh logged in as an admin of the repository.
set -euo pipefail

repo=$(cd "$(dirname "$0")/.." && pwd)
. "$repo/scripts/release-files.sh"
name=LEON_RELEASE_KEY
keys_file="$repo/crates/leon/release-keys.pub"
gh_repo=()
while [ $# -gt 0 ]; do
  case "$1" in
    --name) name=${2:?--name needs a secret name}; shift 2 ;;
    --keys) keys_file=${2:?--keys needs a file}; shift 2 ;;
    --repo) gh_repo=(--repo "${2:?--repo needs OWNER/NAME}"); shift 2 ;;
    -h|--help) sed -n '2,16s/^# \{0,1\}//p' "$0"; exit 0 ;;
    *) echo "release-keys.sh: unknown argument: $1 (try --help)" >&2; exit 2 ;;
  esac
done
printf '%s' "$name" | grep -Eq '^[A-Z][A-Z0-9_]*$' \
  || { echo "release-keys.sh: '$name' is not a secret name (capital letters, digits and _)" >&2; exit 2; }
need minisign gh

existing=$(gh secret list ${gh_repo[@]+"${gh_repo[@]}"} --json name --jq '.[].name')
if printf '%s\n' "$existing" | grep -qxF "$name"; then
  echo "release-keys.sh: the secret $name already exists, and replacing it would throw its key away." >&2
  echo "  To rotate, make a second key under another name: --name ${name}_2 (docs/releases.md#key-rotation)." >&2
  exit 1
fi

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
(umask 077 && minisign -G -W -p "$tmp/key.pub" -s "$tmp/key" >/dev/null)
gh secret set "$name" ${gh_repo[@]+"${gh_repo[@]}"} < "$tmp/key" >/dev/null
public=$(tail -1 "$tmp/key.pub")

# after a last line with no newline, the key would join that line
if [ -s "$keys_file" ] && [ -n "$(tail -c 1 "$keys_file")" ]; then echo >> "$keys_file"; fi
printf '%s\n' "$public" >> "$keys_file"
echo "release-keys.sh: the secret $name is set, and its secret key is deleted from this machine."
echo "The public key, now pinned in $keys_file:"
echo
echo "  $public"
echo
echo "Commit that file: releases are checked against it before they are published."
