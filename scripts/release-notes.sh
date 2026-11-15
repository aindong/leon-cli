#!/usr/bin/env bash
# release-notes.sh: print one version's section of the changelog, the body of its GitHub Release.
#
#   scripts/release-notes.sh VERSION [CHANGELOG]
#
# Prints the lines under `## [VERSION]` up to the next `## [` heading. Exits 1 when the changelog
# [default: CHANGELOG.md] has no such section or the section is empty.
set -euo pipefail

version=${1:?usage: scripts/release-notes.sh VERSION [CHANGELOG]}
changelog=${2:-"$(dirname "$0")/../CHANGELOG.md"}
# index(), not a regex: the dots in a version are dots
notes=$(awk -v head="## [$version]" '
  index($0, "## [") == 1 { on = (index($0, head) == 1) ; next }
  on { print }' "$changelog")
if [ -z "${notes//[[:space:]]/}" ]; then
  echo "release-notes.sh: $changelog has no section for $version (a '## [$version]' heading with text under it)" >&2
  exit 1
fi
# drop the blank lines around the body
printf '%s\n' "$notes" | sed -e '/./,$!d'
