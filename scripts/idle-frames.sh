#!/usr/bin/env bash
# idle-frames.sh: an idle leon app draws nothing. Runs the release app with frame stats on, a
# private runtime dir and an idle /bin/sh for 6 s. Passes only if stats appear while the first
# frames draw (t < 3 s), the app is still alive at the end, and no stats line appears after 3 s.
set -euo pipefail
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
LEON_RUNTIME_DIR="$dir" LEON_FRAME_STATS=1 SHELL=/bin/sh ./target/release/leon app 2>"$log" &
pid=$!
trap 'kill $pid 2>/dev/null || true; wait $pid 2>/dev/null || true; rm -rf "$dir"' EXIT
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
