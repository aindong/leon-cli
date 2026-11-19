#!/usr/bin/env bash
# latest-tag.sh: the tag of leon's latest release, from GitHub's releases/latest JSON on stdin.
#
#   gh api repos/aindong/leon-cli/releases/latest | website/latest-tag.sh
#
# Prints vX.Y.Z and exits 0; anything else exits 1. releases/latest is the newest release that is
# neither a draft nor a prerelease, so it is never the media release; the tag is checked here all
# the same, because site.yml checks it out and puts it in links.
set -euo pipefail
tag=$(jq -r '.tag_name // empty' 2>/dev/null) || tag=
if [[ ! $tag =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "latest-tag.sh: the latest release's tag is not vX.Y.Z: '${tag:-none}'" >&2
  exit 1
fi
echo "$tag"
