#!/usr/bin/env bash
# demo-clips.sh [--only NAME] [OUT_DIR]: record one short clip per feature of the next release in
# the demo VM, each as NAME.gif (at most 1.5 MB, for the changelog and the release notes) and
# NAME.mp4 (for the website's demo page, through the media release: docs/releases.md#cutting-a-release).
# OUT_DIR is target/clips by default; with every scene it also writes demo-app.gif, the four in a
# row, for the README. The scenes (scripts/demo-clips/guest.sh):
#   agents         an agent starts two agents, and leon tells it when they report
#   link-card      resting the pointer on a link, and right-clicking it, shows its card
#   tabs           closing and renaming tabs and sessions
#   cursor-shader  a cursor trail, from scripts/demo-clips/trail.glsl (leon's own, MIT)
# Every take runs in the launch video's VM (scripts/demo-video/README.md: its one-time setup, the
# ssh key in target/demo-video/key): keys and clicks are posted inside the VM, the screen is
# recorded there, and nothing touches this Mac's screen or its own leon. The VM is booted if it
# isn't running, and stopped again after. This Mac builds Leon.app (scripts/bundle.sh, or
# DEMO_APP) and pulls the recordings back, cropped to leon's window. The agents are scripts; no AI
# service runs. Check the clips with `cargo test -p leon --test m13_privacy` once they are in
# docs/images, and a look at the frames.
set -euo pipefail
cd "$(dirname "$0")/.."
repo=$(pwd)
scenes=(agents link-card tabs cursor-shader)
if [ "${1:-}" = --only ]; then
  for s in "${scenes[@]}"; do [ "$s" = "${2:-}" ] && only=$s; done
  [ -n "${only:-}" ] || { echo "demo-clips: --only takes one of: ${scenes[*]}" >&2; exit 2; }
  scenes=("$only")
  shift 2
fi
[ $# -le 1 ] || { echo "usage: scripts/demo-clips.sh [--only NAME] [OUT_DIR]" >&2; exit 2; }
out=${1:-$repo/target/clips}
mkdir -p "$out"
out=$(cd "$out" && pwd)
command -v ffmpeg >/dev/null || { echo "demo-clips: needs ffmpeg (brew install ffmpeg)" >&2; exit 1; }
run=$repo/scripts/demo-video/run-demo.sh
work=${DEMO_WORK:-$repo/target/demo-video}
export DEMO_WORK=$work
[ -f "$work/key" ] || { echo "demo-clips: no ssh key for the VM at $work/key (scripts/demo-video/README.md)" >&2; exit 1; }
app=${DEMO_APP:-$repo/target/release/Leon.app}
[ -x "$app/Contents/MacOS/leon" ] || { echo "demo-clips: no Leon.app at $app: run scripts/bundle.sh" >&2; exit 1; }

# the take's seconds per scene, and the window's frame in the VM's pixels (guest.sh's WINDOW, 2x)
secs() { case $1 in agents) echo 19 ;; link-card) echo 10 ;; tabs) echo 19 ;; cursor-shader) echo 12 ;; esac; }
crop=1800:1360:320:130

ssh_vm() {
  ssh -i "$work/key" -o IdentitiesOnly=yes -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null \
    -o LogLevel=ERROR -o ConnectTimeout=5 "demo@$(cat "$work/.ip")" "$@"
}
booted=
if ! { tart ip "${DEMO_VM:-leon-demo}" >"$work/.ip" 2>/dev/null && ssh_vm true 2>/dev/null; }; then
  "$run" boot
  booted=1
fi
trap 'ssh_vm "bash clips/guest.sh stop" 2>/dev/null || true; [ -z "$booted" ] || "$run" stop' EXIT
DEMO_APP=$app "$run" push
COPYFILE_DISABLE=1 tar -C "$repo/scripts/demo-clips" -cf - . | ssh_vm 'rm -rf ~/clips && mkdir ~/clips && tar -xf - -C ~/clips'

gif() { # gif IN OUT [MAX]: at most MAX bytes (1.5 MB), the widest that fits; prints the width
  local width max=${3:-1500000}
  for width in 1080 960 840 720; do
    ffmpeg -loglevel error -y -i "$1" -vf \
      "fps=12,scale=$width:-1:flags=lanczos,mpdecimate,split[a][b];[a]palettegen=max_colors=96:stats_mode=diff[p];[b][p]paletteuse=dither=none:diff_mode=rectangle" \
      -loop 0 "$2"
    [ "$(wc -c <"$2")" -le "$max" ] && { echo "$width"; return; }
  done
  echo "demo-clips: $2 is over $max bytes even at ${width}px" >&2
  return 1
}

for s in "${scenes[@]}"; do
  ssh_vm "bash clips/guest.sh stage $s"
  # screencapture hangs when started in the background inside the VM: it runs in the foreground
  # of its own ssh session, which is in the background here (run-demo.sh's take does the same)
  ssh_vm "rm -f ~/clip.mov; screencapture -v -V $(secs "$s") -x ~/clip.mov </dev/null" &
  rec=$!
  # a still second ends the VM's recorder early: the parked pointer steps on the screen's right
  # edge, outside the window (run-demo.sh's take does the same); not while the pointer is in use
  wig=
  if [ "$s" != link-card ]; then
    ssh_vm "kit/bin/keypost wiggle $(secs "$s")" &
    wig=$!
  fi
  sleep 1.5
  ssh_vm "bash clips/guest.sh play $s"
  wait "$rec" $wig
  ssh_vm "bash clips/guest.sh stop"
  ssh_vm 'cat ~/clip.mov' >"$work/clip-$s.mov"
  # the recorder's first second is the window standing still: drop it
  # the recording's frame rate varies with what changes: 30 a second, evenly, for the clip; and
  # no longer than the recording, whose last frame can carry a bad timestamp (120 s on a 10 s take)
  long=$(ffprobe -v error -show_entries format=duration -of csv=p=0 "$work/clip-$s.mov")
  ffmpeg -loglevel error -y -ss 1.2 -i "$work/clip-$s.mov" -t "$(python3 -c "print(round($long - 1.2, 2))")" \
    -vf "fps=30,crop=$crop,scale=1600:-2:flags=lanczos,format=yuv420p" \
    -c:v libx264 -crf 22 -preset slow -movflags +faststart -an "$out/$s.mp4"
  width=$(gif "$out/$s.mp4" "$out/$s.gif")
  echo "demo-clips: $s.gif ($(($(wc -c <"$out/$s.gif") / 1024)) KB, ${width}px) and $s.mp4"
done
# the README's GIF: every clip, one after the other (as large as the one it replaced)
if [ ${#scenes[@]} -eq 4 ]; then
  for s in "${scenes[@]}"; do echo "file '$out/$s.mp4'"; done >"$work/clips.txt"
  ffmpeg -loglevel error -y -f concat -safe 0 -i "$work/clips.txt" -c copy "$work/demo-app.mp4"
  width=$(gif "$work/demo-app.mp4" "$out/demo-app.gif" 2500000)
  echo "demo-clips: demo-app.gif ($(($(wc -c <"$out/demo-app.gif") / 1024)) KB, ${width}px)"
fi
