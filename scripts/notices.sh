#!/usr/bin/env bash
# notices.sh: print the third-party notices for the crates built into the leon binary: each crate
# with its license, then each distinct license file once, with the crates that ship it.
#
#   scripts/notices.sh [--target TRIPLE]
#
# --target TRIPLE  the platform to list crates for [default: this machine's]
#
# The crates come from `cargo tree` (normal and build dependencies, since a build dependency such as
# lua-src compiles C code into the binary); their files from the cargo registry, or cargo's git
# checkout for a git dependency (GPUI's fork, docs/architecture/gpui-patch.md), so build first.
set -euo pipefail
cd "$(dirname "$0")/.."
target=()
while [ $# -gt 0 ]; do
  case "$1" in
    --target) target=(--target "${2:?--target needs a triple}"); shift 2 ;;
    -h|--help) sed -n '2,10s/^# \{0,1\}//p' "$0"; exit 0 ;;
    *) echo "notices.sh: unknown argument: $1 (try --help)" >&2; exit 2 ;;
  esac
done

registry="${CARGO_HOME:-$HOME/.cargo}/registry/src"
checkouts="${CARGO_HOME:-$HOME/.cargo}/git/checkouts"
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
# "name vX.Y.Z|license|repository"; workspace crates (path dependencies) are leon's own LICENSE
cargo tree --locked --offline -p leon -e normal,build --prefix none --format '{p}|{l}|{r}' ${target[@]+"${target[@]}"} \
  | sed 's/ (\*)$//; s/ (proc-macro)//' | grep -v ' (/' | sort -u > "$tmp/tree"
# a git dependency reads "name vX.Y.Z (https://…#<short commit>)": "name-X.Y.Z name <short commit>"
# for finding its checkout, and the line as a registry crate's
sed -n 's/^\([^ ]*\) v\([^ |]*\) ([^)]*#\([0-9a-f]*\)).*/\1-\2 \1 \3/p' "$tmp/tree" > "$tmp/git"
sed 's/^\([^ ]*\) v\([^ |]*\) ([^)]*#[0-9a-f]*)/\1 v\2/' "$tmp/tree" | sort -u > "$tmp/crates"
[ -s "$tmp/crates" ] || { echo "notices.sh: cargo tree listed no crates" >&2; exit 1; }
# each crate's package dir, then every license file in them with its checksum, in one pass each
sed 's/^\(.*\) v\([^|]*\)|.*/\1-\2/' "$tmp/crates" | while read -r d; do
  grep -q "^$d " "$tmp/git" && continue # its files are its checkout's (below), not an old download's
  for p in "$registry"/*/"$d"; do if [ -d "$p" ]; then echo "$p"; fi; done
done > "$tmp/dirs"
# a git dependency's checkout (<repo>-<hash>/<short commit>), linked as name-X.Y.Z like the rest
mkdir "$tmp/git-pkgs"
while read -r d name short; do
  for p in "$checkouts"/*/*; do
    case "$short" in "$(basename "$p")"*) ;; *) continue ;; esac
    if grep -q "^name = \"$name\"" "$p/Cargo.toml" 2>/dev/null; then
      ln -s "$p" "$tmp/git-pkgs/$d" && echo "$tmp/git-pkgs/$d" >> "$tmp/dirs"
      break
    fi
  done
  [ -e "$tmp/git-pkgs/$d" ] || echo "notices.sh: no checkout of $d ($short) under $checkouts: build first" >&2
done < "$tmp/git"
tr '\n' '\0' < "$tmp/dirs" | xargs -0 -I{} find -H {} -maxdepth 1 -type f \( -iname 'licen[cs]e*' \
  -o -iname 'copying*' -o -iname 'notice*' -o -iname 'unlicense*' \) -print0 | xargs -0 cksum | sort -k3 > "$tmp/sums"

version=$(sed -n 's/^version = "\(.*\)"$/\1/p' Cargo.toml | head -1)
cat <<EOF
leon $version includes the third-party Rust crates below. Each is listed with the license it
declares; then each license file from the crates' packages follows once, with the crates that
ship it. A crate marked "no license file" ships none in its package: its license is the one
named, and its repository has the text.

EOF
# $tmp/sums lines: "<sum> <size> <dir>/<name>-<ver>/<file>"
awk -F'|' -v sums="$tmp/sums" '
  BEGIN { while ((getline line < sums) > 0) { sub(/^[0-9]+ [0-9]+ /, "", line); n = split(line, p, "/")
            has[p[n-1]] = 1 } }
  { name = $1; sub(/ v[^ ]*$/, "", name); ver = $1; sub(/^.* v/, "", ver)
    if (has[name "-" ver]) print "- " name " " ver ": " ($2 == "" ? "see its license file" : $2)
    else print "- " name " " ver ": " ($2 == "" ? "no license declared" : $2) " (no license file; " ($3 == "" ? "no repository listed" : $3) ")" }
' "$tmp/crates"
# group the files by checksum: who ships each text, then the text once
awk '{ key = $1 "_" $2; path = $0; sub(/^[0-9]+ [0-9]+ /, "", path); n = split(path, p, "/")
       who[key] = who[key] "  " p[n-1] " (" p[n] ")\\n"
       if (!(key in file)) { file[key] = path; order[++k] = key } }
     END { for (i = 1; i <= k; i++) print order[i] "\t" file[order[i]] "\t" who[order[i]] }' "$tmp/sums" \
  | while IFS=$'\t' read -r _ f who; do
  printf '\n%s\nShipped by:\n%b%s\n' "================================================================================" \
    "$who" "--------------------------------------------------------------------------------"
  cat "$f"
done
