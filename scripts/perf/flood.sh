#!/usr/bin/env bash
# flood.sh: frames while panes print as fast as they can, and how long until the screen is current.
#   scripts/perf/flood.sh [--panes "1 8"] [--mb 100] [--quiet K]
# Per count: a session of N panes, each waiting for a go file, then stamping the boot clock,
# `cat`ing the flood file (mixed text and 16-colour SGR, scripts/perf/mkflood.py), stamping again
# and printing a sentinel bar. The app runs with LEON_FRAME_LOG; winframes watches its window until
# N sentinel bars show; a small window of winframes' own, redrawn every display frame, is watched
# at the same time: its frame rate is what the capture itself delivered under that load. With
# --quiet K, K more panes beside them stay quiet (what a cached grid skips).
source "$(dirname "$0")/lib.sh"
counts="1 8" mb=100 quiet=0
while [ $# -gt 0 ]; do
  case "$1" in
    --panes) counts=$2; shift 2 ;;
    --mb) mb=$2; shift 2 ;;
    --quiet) quiet=$2; shift 2 ;;
    *) echo "usage: flood.sh [--panes \"1 8\"] [--mb MB] [--quiet K]" >&2; exit 2 ;;
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
  cat >"$RUN/pane-$n.sh" <<SH
while [ ! -e '$go' ]; do sleep 0.02; done
'$PERF_PY' -c '$stamp' begin >>'$marks'; cat '$flood'; '$PERF_PY' -c '$stamp' end >>'$marks'
printf '\033[48;2;255;0;128m%60s\033[0m\n' ''; exec sleep 100000
SH
  # the first N panes to claim a slot flood; the rest stay quiet
  mkdir "$RUN/claims-$n"
  stack "$sid" "$(first_block "$sid")" "$((n + quiet))" \
    "for i in \$(seq $n); do mkdir '$RUN/claims-$n/'\$i 2>/dev/null && exec sh '$RUN/pane-$n.sh'; done; exec sleep 100000" >/dev/null
  perf_app "$RUN/$n-frames.log"
  "$WINFRAMES" floor 200 120 animate >/dev/null &
  floor=$!
  PERF_PIDS+=("$floor")
  sleep 2
  "$WINFRAMES" "$APP_PID" watch 120 "$n" "floor=$floor" >"$RUN/$n-watch.txt" &
  watch=$!
  PERF_PIDS+=("$watch")
  # go once both captures run
  for i in $(seq 250); do grep -q '^start ' "$RUN/$n-watch.txt" && break; sleep 0.1; done
  sleep 0.5
  touch "$go"
  wait "$watch" || true
  perf_stop "$floor"
  perf_stop "$APP_PID"
  "$LEON" kill "$sid" >/dev/null
  echo "== $n pane(s), $mb MB each, $quiet quiet (load=$(load1))" | tee -a "$RUN/summary.txt"
  "$PERF_PY" "$PERF_ROOT/scripts/perf/stats.py" flood "$RUN/$n-watch.txt" "$RUN/$n-frames.log" "$marks" "$n" | tee -a "$RUN/summary.txt"
done
perf_note "end"
echo "flood: $RUN/summary.txt"
