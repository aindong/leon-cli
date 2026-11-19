#!/usr/bin/env bash
# brand/marketing/media.sh: the landing page's pictures, made from the docs' own (docs/images/) at
# the sizes the page shows them, so the page stays light on a phone. Needs cwebp and ffmpeg. The
# app icon beside them, app-icon.png, comes from brand/build-assets.cjs.
set -euo pipefail
cd "$(dirname "$0")/../.."
out=brand/marketing
cwebp -quiet -q 82 docs/images/app-status.png -o "$out/app.webp"
cwebp -quiet -q 82 docs/images/app-overview.png -o "$out/overview.webp"
# the demos as videos: a fraction of the GIFs' bytes, and the visitor can pause them. The poster is a
# frame from the middle, where there is something to see.
for demo in app:6 cli:20; do
  name=demo-${demo%:*} at=${demo#*:}
  ffmpeg -v error -y -i "docs/images/$name.gif" -an -movflags +faststart -pix_fmt yuv420p \
    -vf "scale=trunc(iw/2)*2:trunc(ih/2)*2" -c:v libx264 -crf 26 -preset slow "$out/$name.mp4"
  ffmpeg -v error -y -ss "$at" -i "docs/images/$name.gif" -frames:v 1 "$out/$name.png"
  cwebp -quiet -q 82 "$out/$name.png" -o "$out/$name.webp"
  rm "$out/$name.png"
done
