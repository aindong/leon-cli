#!/usr/bin/env bash
# fetch-media.sh DIR: download the demo videos and their posters from the `media` release into DIR,
# for website/build.sh (LEON_SITE_MEDIA=DIR). Large media never go in git, and never into a
# versioned release (release-verify.sh and the updater read those): they live in that one release,
# a prerelease, so releases/latest never picks it. With no `media` release, DIR stays empty and
# the demo page shows the app's GIFs instead. Needs gh and jq, with GH_TOKEN and GH_REPO set (site.yml sets them).
set -euo pipefail
dir=${1:?usage: website/fetch-media.sh DIR}
mkdir -p "$dir"
err=$(mktemp)
trap 'rm -f "$err"' EXIT
if ! info=$(gh release view media --json isPrerelease,assets 2>"$err"); then
  case $(cat "$err") in
    *"release not found"*) echo "fetch-media.sh: no media release, so no videos"; exit 0 ;;
  esac
  cat "$err" >&2
  exit 1
fi
if [ "$(jq -r .isPrerelease <<<"$info")" != true ]; then
  echo "fetch-media.sh: the media release must be a prerelease, or it could become releases/latest (docs/releases.md#the-website)" >&2
  exit 1
fi
patterns=('*.mp4' '*.webm' '*.jpg' '*.png')
if [ "$(jq '[.assets[].name | select(test("\\.(mp4|webm|jpg|png)$"))] | length' <<<"$info")" = 0 ]; then
  echo "fetch-media.sh: the media release holds no videos or posters yet"
  exit 0
fi
args=()
for p in "${patterns[@]}"; do args+=(--pattern "$p"); done
gh release download media --dir "$dir" --clobber "${args[@]}"
ls -l "$dir"
