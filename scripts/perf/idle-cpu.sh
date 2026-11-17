#!/usr/bin/env bash
# idle-cpu.sh: CPU the server and the app use when nothing changes (docs/performance.md).
#   scripts/perf/idle-cpu.sh [--secs 300]
# Twelve idle /bin/sh panes in the shown tab, then three states of SECS each: plain; one pane
# reporting `working` (its status indicator pulses); the app hidden (as ⌘H does). Per state: CPU
# seconds used (ps), as a percentage of one core, idle wake-ups (top's IDLEW), and the app's frames.
source "$(dirname "$0")/lib.sh"
secs=300
[ "${1:-}" = --secs ] && secs=$2
perf_screen_gate
perf_init idle-cpu
perf_server
sid=$("$LEON" new idle)
stack "$sid" "$(first_block "$sid")" 12 "exec /bin/sh" >/dev/null
perf_app "$RUN/frames.log"
wakeups() { top -l 1 -pid "$1" -stats idlew | tail -1 | tr -d ' +'; }
measure() {
  perf_load_gate
  perf_note "idle-cpu $1"
  sleep 5 # past whatever the change itself drew
  local s0 a0 sw0 aw0 s1 a1 sw1 aw1
  local u0 u1
  s0=$(cpu_s "$SERVER_PID") a0=$(cpu_s "$APP_PID") sw0=$(wakeups "$SERVER_PID") aw0=$(wakeups "$APP_PID") u0=$(uptime_ns)
  sleep "$secs"
  s1=$(cpu_s "$SERVER_PID") a1=$(cpu_s "$APP_PID") sw1=$(wakeups "$SERVER_PID") aw1=$(wakeups "$APP_PID") u1=$(uptime_ns)
  local frames
  frames=$(awk -v a="$u0" -v b="$u1" '$1 == "frame" && $2 >= a && $2 < b { n++ } END { print n + 0 }' "$RUN/frames.log")
  awk -v n="$1" -v t="$secs" -v l="$(load1)" -v f="$frames" \
    -v ds="$(echo "$s1 - $s0" | bc)" -v da="$(echo "$a1 - $a0" | bc)" -v ws="$((sw1 - sw0))" -v wa="$((aw1 - aw0))" \
    'BEGIN { printf "%s (%d s, load=%s): server %.2f%% of a core, %.1f wakeups/s; app %.2f%% of a core, %.1f wakeups/s, %.1f frames/s\n", n, t, l, 100 * ds / t, ws / t, 100 * da / t, wa / t, f / t }' |
    tee -a "$RUN/summary.txt"
}
measure "12 idle panes"
blk=$("$LEON" --json block ls -s "$sid" | "$PERF_PY" -c 'import json,sys; print(json.load(sys.stdin)[0]["block_id"])')
"$LEON" send -s "$sid" -b "$blk" "printf '\\033]7501;state=working\\007'"
"$LEON" send-key -s "$sid" -b "$blk" enter
measure "one pane working (indicator pulsing)"
"$WINFRAMES" "$APP_PID" hide
measure "app hidden"
"$WINFRAMES" "$APP_PID" unhide
perf_note "end"
