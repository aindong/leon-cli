#!/usr/bin/env bash
# release-notes.sh: print one version's section of the changelog, the body of its GitHub Release.
#
#   scripts/release-notes.sh VERSION [CHANGELOG]
#
# Prints the lines under `## [VERSION]` up to the next `## [` heading, with its relative links made
# absolute to the tag vVERSION on GitHub (a Release page can't resolve them). Without CHANGELOG, an
# earlier release's section comes from docs/changelog-archive.md, whose links start from docs/.
# Exits 1 when the changelog [default: CHANGELOG.md] has no such section or the section is empty.
set -euo pipefail

case "${1:-}" in -h|--help) sed -n '2,7s/^# \{0,1\}//p' "$0"; exit 0 ;; esac
version=${1:?usage: scripts/release-notes.sh VERSION [CHANGELOG]}
changelog=${2:-"$(dirname "$0")/../CHANGELOG.md"}
# index(), not a regex: the dots in a version are dots
section() {
  awk -v head="## [$version]" '
    index($0, "## [") == 1 { on = (index($0, head) == 1) ; next }
    on { print }' "$1"
}
notes=$(section "$changelog")
archive="$(dirname "$0")/../docs/changelog-archive.md"
if [ -z "$(printf '%s' "$notes" | tr -d '[:space:]')" ] && [ -z "${2:-}" ] && [ -f "$archive" ]; then
  # its links start from docs/: as CHANGELOG.md's again, which is where they were at the tag
  notes=$(section "$archive" | sed -E \
    -e 's#\]\(([^):\#[:space:]][^):[:space:]]*)\)#](docs/\1)#g' \
    -e 's#^(\[[^]]+\]:[[:space:]]+)([^:\#[:space:]][^:[:space:]]*)([[:space:]].*)?$#\1docs/\2\3#' \
    -e 's#docs/\.\./##g')
fi
# tr, not ${notes//[[:space:]]/}: bash 3.2 (macOS) takes ten seconds over a 6 KB section that way
if [ -z "$(printf '%s' "$notes" | tr -d '[:space:]')" ]; then
  echo "release-notes.sh: $changelog has no section for $version (a '## [$version]' heading with text under it)" >&2
  exit 1
fi
# drop the blank lines around the body; then images, links, anchors in CHANGELOG.md and reference
# definitions with no scheme (no `:`) go to the tag: a picture by its raw file
tag=https://github.com/aindong/leon-cli
printf '%s\n' "$notes" | sed -E -e '/./,$!d' \
  -e "s|(!\[[^]]*\])\(([^):#[:space:]][^):[:space:]]*)\)|\1($tag/raw/v$version/\2)|g" \
  -e "s|\]\(([^):#[:space:]][^):[:space:]]*)\)|]($tag/blob/v$version/\1)|g" \
  -e "s|\]\(#|]($tag/blob/v$version/CHANGELOG.md#|g" \
  -e "s|^(\[[^]]+\]:[[:space:]]+)([^:#[:space:]][^:[:space:]]*)([[:space:]].*)?$|\1$tag/blob/v$version/\2\3|" \
  -e "s|^(\[[^]]+\]:[[:space:]]+)#|\1$tag/blob/v$version/CHANGELOG.md#|"
