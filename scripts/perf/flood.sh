#!/usr/bin/env bash
# flood.sh: frames while panes print as fast as they can, and how long until the screen is current.
#   scripts/perf/flood.sh [--panes "1 8"] [--mb 100]
# Per count: a session of N panes, each waiting for a go file, then stamping the boot clock,
# `cat`ing the flood file (mixed text and 16-colour SGR, scripts/perf/mkflood.py), stamping again
# and printing a sentinel bar. The app runs with LEON_FRAME_LOG; winframes watches its window until
# N sentinel bars show; a small window of winframes' own, redrawn every display frame, is watched
# at the same time: its frame rate is what the capture itself delivered under that load.
source "$(dirname "$0")/lib.sh"
counts="1 8" mb=100
while [ $# -gt 0 ]; do
  case "$1" in
    --panes) counts=$2; shift 2 ;;
    --mb) mb=$2; shift 2 ;;
    *) echo "usage: flood.sh [--panes \"1 8\"] [--mb MB]" >&2; exit 2 ;;
  esac
done
perf_screen_gate
perf_init flood
flood="$PERF_ROOT/target/perf/flood-$mb.txt"
[ -s "$flood" ] || "$PERF_PY" "$PERF_ROOT/scripts/perf/mkflood.py" "$flood" "$mb"
stamp="import time,sys; print(sys.argv[1], time.clock_gettime_ns(time.CLOCK_UPTIME_RAW))"
perf_server
for n in $counts; do
  perf_load_gate
  perf_note "flood $n"
  go="$RUN/go-$n" marks="$RUN/$n-marks.txt"
  sid=$("$LEON" new "flood-$n")
  stack "$sid" "$(first_block "$sid")" "$n" "while [ ! -e '$go' ]; do sleep 0.02; done
    '$PERF_PY' -c '$stamp' begin >>'$marks'; cat '$flood'; '$PERF_PY' -c '$stamp' end >>'$marks'
    printf '\033[48;2;255;0;128m%60s\033[0m\n' ''; exec sleep 100000" >/dev/null
  perf_app "$RUN/$n-frames.log"
  "$WINFRAMES" floor 200 120 animate >/dev/null &
  floor=$!
  PERF_PIDS+=("$floor")
  sleep 2
  "$WINFRAMES" "$APP_PID" watch 120 "$n" "floor=$floor" >"$RUN/$n-watch.txt" &
  watch=$!
  # go once both captures run
  for i in $(seq 250); do grep -q '^start ' "$RUN/$n-watch.txt" && break; sleep 0.1; done
  sleep 0.5
  touch "$go"
  wait "$watch"
  kill "$floor" 2>/dev/null || true
  kill "$APP_PID"; wait "$APP_PID" 2>/dev/null || true
  "$LEON" kill "$sid" >/dev/null
  echo "== $n pane(s), $mb MB each (load=$(load1))" | tee -a "$RUN/summary.txt"
  "$PERF_PY" "$PERF_ROOT/scripts/perf/stats.py" flood "$RUN/$n-watch.txt" "$RUN/$n-frames.log" "$marks" "$n" | tee -a "$RUN/summary.txt"
done
perf_note "end"
echo "flood: $RUN/summary.txt"
