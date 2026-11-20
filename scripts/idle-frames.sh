#!/usr/bin/env bash
# idle-frames.sh [--overview | --cursor-shader FILE]: an idle leon app draws nothing. Runs the release
# app with frame stats on, a private runtime and state dir and an idle /bin/sh for 6 s. Passes only if
# stats appear while the first frames draw (t < 3 s), the app is still alive at the end, and no stats
# line appears after 3 s. With --overview it then sends ⌘⇧\ through System Events (the terminal
# running this needs Accessibility), requires the app's own word that the overview opened, and the
# same 3 s of quiet. With --cursor-shader it runs with an init.lua naming FILE as the cursor shader,
# brings the window to the front, moves the cursor with `leon send`, requires the overlay's own
# stats line for that animation, and then 3 s with no frame of GPUI's or the overlay's.
set -euo pipefail
overview=false
shader=""
case "${1:-}" in
  --overview) overview=true ;;
  --cursor-shader) shader=$(cd "$(dirname "${2:?--cursor-shader takes a shader file}")" && pwd)/$(basename "$2") ;;
esac
cd "$(dirname "$0")/.."
# A locked screen marks every window occluded and GPUI stops drawing, so "no frames" would prove nothing.
locked=$(swift - 2>/dev/null <<'SWIFT' || echo 0
import CoreGraphics
let d = CGSessionCopyCurrentDictionary() as? [String: Any] ?? [:]
print(d["CGSSessionScreenIsLocked"] as? Int ?? 0)
SWIFT
)
if [ "$locked" = 1 ]; then
  echo "idle-frames: the screen is locked; can't measure drawing (exit 2)"
  exit 2
fi
cargo build --release -q -p leon
dir=$(mktemp -d)
chmod 700 "$dir"
log="$dir/stats.log"
# Private dirs throughout: the app starts a server that outlives it (and would otherwise restore
# and save the real ~/.local/state/leon), so cleanup stops that server before removing them.
export LEON_RUNTIME_DIR="$dir" LEON_STATE_DIR="$dir/state" LEON_CONFIG="$dir/no-init.lua"
if [ -n "$shader" ]; then
  [ -f "$shader" ] || { echo "idle-frames: no shader file $shader"; exit 2; }
  printf 'leon.set("cursor_shader", [[%s]])\nleon.set("cursor_shader_duration", 0.5)\n' "$shader" >"$dir/init.lua"
  export LEON_CONFIG="$dir/init.lua"
fi
# the app's daily look for a release goes to a loopback port that refuses: it fails silently and
# draws nothing, and nothing reaches the real release server
export LEON_RELEASE_URL=http://127.0.0.1:9
LEON_FRAME_STATS=1 SHELL=/bin/sh ./target/release/leon app 2>"$log" &
pid=$!
trap './target/release/leon --autostart=false server stop >/dev/null 2>&1 || true; kill $pid 2>/dev/null || true; wait $pid 2>/dev/null || true; rm -rf "$dir"' EXIT
fail() { echo "idle-frames: FAIL: $*"; cat "$log"; exit 1; }
sleep 3
early=$(grep -c '^leon frames' "$log" || true)
[ "$early" -ge 1 ] || fail "no frame stats in the first 3 s"
sleep 3
kill -0 "$pid" 2>/dev/null || fail "the app exited"
late=$(grep -c '^leon frames' "$log" || true)
[ "$late" -eq "$early" ] || fail "an idle app drew frames after 3 s ($early lines, then $late)"
echo "idle-frames: ok ($early stats line(s) while starting, none while idle)"
grep '^leon frames' "$log"
frames() { grep -c -e '^leon frames' -e '^leon cursor-shader' "$log" || true; }
if [ -n "$shader" ]; then
  osascript -e "tell application \"System Events\" to set frontmost of (first process whose unix id is $pid) to true" >/dev/null
  sleep 1
  ./target/release/leon --autostart=false send 'echo moved'
  sleep 3
  grep -q '^leon cursor-shader' "$log" || fail "the overlay drew no animation (no 'leon cursor-shader' line)"
  grep -q '^leon: cursor shader' "$log" && fail "the shader didn't load"
  early=$(frames)
  sleep 3
  kill -0 "$pid" 2>/dev/null || fail "the app exited"
  late=$(frames)
  [ "$late" -eq "$early" ] || fail "frames after the animation ended ($early lines, then $late)"
  echo "idle-frames: ok with a cursor shader (it animated, then nothing while idle)"
  grep '^leon cursor-shader' "$log"
fi
if $overview; then
  osascript -e "tell application \"System Events\" to set frontmost of (first process whose unix id is $pid) to true" >/dev/null
  sleep 0.5
  osascript -e 'tell application "System Events" to keystroke "\\" using {command down, shift down}'
  sleep 3
  grep -q '^leon overview: open' "$log" || fail "the overview didn't open (no 'leon overview: open' line)"
  early=$(grep -c '^leon frames' "$log" || true)
  [ "$early" -gt "$late" ] || fail "no frames were drawn when the overview opened"
  sleep 3
  kill -0 "$pid" 2>/dev/null || fail "the app exited"
  late=$(grep -c '^leon frames' "$log" || true)
  [ "$late" -eq "$early" ] || fail "an idle overview drew frames after 3 s ($early lines, then $late)"
  echo "idle-frames: ok with the overview open (none while idle)"
fi
