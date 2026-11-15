#!/usr/bin/env bash
# release-verify.sh: check a directory of release files against its sums, manifest and signatures.
#
#   scripts/release-verify.sh [--complete] [--keys FILE] DIR
#
# DIR holds a release as scripts/release-manifest.sh left it, or as downloaded. Exits 0 only if
# latest.json and SHA256SUMS are signed by a trusted key, the manifest has the documented fields
# (docs/releases.md#the-manifest), and every file in DIR is in the manifest with its size, its
# sha256 and a signature by a trusted key. Any other file in DIR is an error.
#
# --complete   also fail unless the release has all eight files of a full release
# --keys FILE  the trusted minisign public keys, one per line [default: crates/leon/release-keys.pub,
#              the keys leon pins]
#
# Needs minisign and jq.
set -euo pipefail

repo=$(cd "$(dirname "$0")/.." && pwd)
. "$repo/scripts/release-files.sh"
complete=""
keys_file="$repo/crates/leon/release-keys.pub"
dir=""
while [ $# -gt 0 ]; do
  case "$1" in
    --complete) complete=1; shift ;;
    --keys) keys_file=${2:?--keys needs a file}; shift 2 ;;
    -h|--help) sed -n '2,15s/^# \{0,1\}//p' "$0"; exit 0 ;;
    -*) echo "release-verify.sh: unknown argument: $1 (try --help)" >&2; exit 2 ;;
    *) [ -z "$dir" ] || { echo "release-verify.sh: one directory, not also $1" >&2; exit 2; }; dir=$1; shift ;;
  esac
done
[ -n "$dir" ] && [ -d "$dir" ] || { echo "release-verify.sh: give the directory of release files (try --help)" >&2; exit 2; }
need minisign jq
fail() { echo "release-verify.sh: FAIL: $*" >&2; exit 1; }

keys=()
while read -r line || [ -n "$line" ]; do # a last line with no newline is still a key
  case "$line" in ''|'#'*) ;; *) keys+=("$line") ;; esac
done < "$keys_file"
[ ${#keys[@]} -gt 0 ] || fail "$keys_file pins no key: run scripts/release-keys.sh (docs/releases.md#secrets)"

# `signed NAME`: NAME's .minisig verifies against one of the trusted keys
signed() {
  local key
  [ -f "$dir/$1" ] || fail "$1 is missing"
  [ -f "$dir/$1.minisig" ] || fail "$1 has no signature ($1.minisig is missing)"
  for key in "${keys[@]}"; do
    if minisign -V -q -P "$key" -m "$dir/$1" 2>/dev/null; then return 0; fi
  done
  fail "$1: its signature is not by a key in $keys_file, or the file was changed"
}

# the two lists first: nothing in them is believed until their signatures hold
signed latest.json
signed SHA256SUMS
manifest="$dir/latest.json"
jq -e '
  def str: type == "string" and length > 0;
  def line: str and (contains("\n") | not); # a regex $ also matches before a last newline
  .schema == 1 and (.version | line) and .tag == "v" + .version and (.commit | line)
  and (.date | line and test("^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z$"))
  and (.notes_url | line) and (.min_os | type == "object" and all(.[]; line))
  and (.files | type == "array" and length > 0)
  and all(.files[];
    (.name | line and test("^[A-Za-z0-9._-]+$")) and (.kind | IN("app", "cli", "deb"))
    and (.os | IN("macos", "linux")) and (.arch | IN("arm64", "x86_64"))
    and (.name as $n | .url | line and endswith("/" + $n)) # in parentheses: jq 1.8 binds `as` wider than 1.7
    and (.size | type == "number") and (.sha256 | line and test("^[0-9a-f]{64}$")) and (.signature | str))
' "$manifest" >/dev/null || fail "latest.json does not have the fields docs/releases.md documents"

version=$(jq -r .version "$manifest")
listed=$(jq -r '.files[].name' "$manifest")
# `is_listed NAME`: NAME is exactly one of the manifest's files (their names have no spaces)
is_listed() {
  local n
  for n in $listed; do [ "$n" = "$1" ] && return 0; done
  return 1
}
shopt -s dotglob nullglob # hidden files count as files too
for path in "$dir"/*; do
  name=${path##*/}
  case "$name" in
    latest.json|SHA256SUMS|latest.json.minisig|SHA256SUMS.minisig) continue ;;
    *.minisig) is_listed "${name%.minisig}" || fail "$name is the signature of no file in the manifest"; continue ;;
  esac
  is_listed "$name" || fail "$name is not in the manifest: nobody signed it"
done
shopt -u dotglob nullglob
[ "$(awk '{ print $2 }' "$dir/SHA256SUMS" | sort)" = "$(printf '%s\n' "$listed" | sort)" ] \
  || fail "SHA256SUMS and latest.json list different files"

for name in $listed; do
  signed "$name"
  sum=$(sha256 "$dir/$name" | cut -d' ' -f1)
  [ "$sum" = "$(awk -v n="$name" '$2 == n { print $1 }' "$dir/SHA256SUMS")" ] || fail "$name: its sha256 is not the one in SHA256SUMS"
  jq -e --arg n "$name" --arg sum "$sum" --argjson size "$(wc -c < "$dir/$name" | tr -d ' ')" \
    --rawfile sig "$dir/$name.minisig" \
    '.files[] | select(.name == $n) | .sha256 == $sum and .size == $size and .signature == $sig' "$manifest" >/dev/null \
    || fail "$name: its size, sha256 or signature is not the one in latest.json"
done

if [ -n "$complete" ]; then
  while read -r name _; do
    is_listed "$name" || fail "--complete, but the release has no $name"
  done <<<"$(release_files "$version")"
fi
echo "release-verify.sh: ok: leon $version, $(printf '%s\n' "$listed" | wc -l | tr -d ' ') files: sums, manifest and signatures hold"
