#!/usr/bin/env bash
# keylat.sh: key to pixel in the app, by two clocks, under three loads (docs/performance.md).
#   scripts/perf/keylat.sh [--loads "idle flood1 busy8"] [--keys N] [--delay MS]
# Per load: a fresh session whose focused pane runs scripts/perf/echo.py (plus one pane looping
# `cat` of the 100 MB flood file, or eight panes of agent.py busy), the app with LEON_FRAME_LOG,
# and winframes posting N keys to the app's pid. --delay makes echo.py wait MS before each echo:
# both clocks must rise by it (the check that they measure the echo).
source "$(dirname "$0")/lib.sh"
loads="idle flood1 busy8" keys=200 delay=0
while [ $# -gt 0 ]; do
  case "$1" in
    --loads) loads=$2; shift 2 ;;
    --keys) keys=$2; shift 2 ;;
    --delay) delay=$2; shift 2 ;;
    *) echo "usage: keylat.sh [--loads L] [--keys N] [--delay MS]" >&2; exit 2 ;;
  esac
done
perf_screen_gate
perf_init keylat
flood="$PERF_ROOT/target/perf/flood-100.txt"
[ -s "$flood" ] || "$PERF_PY" "$PERF_ROOT/scripts/perf/mkflood.py" "$flood" 100
perf_server
for load in $loads; do
  perf_load_gate
  perf_note "keylat $load"
  sid=$("$LEON" new "kl-$load")
  echo_id=$("$LEON" split -s "$sid" --label echo -- "$PERF_PY" "$PERF_ROOT/scripts/perf/echo.py" --delay "$delay")
  case "$load" in
    idle) ;;
    flood1) "$LEON" split -s "$sid" -b "$echo_id" --split below --focus=false -- sh -c "while :; do cat '$flood'; done" >/dev/null ;;
    busy8) for i in 1 2 3 4 5 6 7 8; do
        "$LEON" split -s "$sid" -b "$echo_id" --split below --ratio 80 --focus=false -- "$PERF_PY" "$PERF_ROOT/scripts/perf/agent.py" busy >/dev/null
      done ;;
    *) echo "unknown load $load" >&2; exit 2 ;;
  esac
  perf_app "$RUN/$load-frames.log"
  sleep 2 # past start-up, and past GPUI's "present every frame for 1 s after input"
  "$WINFRAMES" "$APP_PID" keys "$keys" 50 >"$RUN/$load-winframes.txt"
  kill "$APP_PID"; wait "$APP_PID" 2>/dev/null || true
  "$LEON" kill "$sid" >/dev/null
  echo "== $load (load=$(load1), delay=${delay} ms)" | tee -a "$RUN/summary.txt"
  "$PERF_PY" "$PERF_ROOT/scripts/perf/stats.py" keylat "$RUN/$load-winframes.txt" "$RUN/$load-frames.log" | tee -a "$RUN/summary.txt"
done
perf_note "end"
echo "keylat: $RUN/summary.txt"
