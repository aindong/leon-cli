#!/usr/bin/env bash
# overview.sh: how long the app's overview (⌘⇧\) takes to open with many tabs (docs/performance-app.md).
#   scripts/perf/overview.sh [--runs N] [--tabs N]
# TABS tabs (default 50) in 10 sessions, each a shell that printed a screenful, then the release
# app with its frame log, and ⌘⇧\ through System Events (the terminal running this needs
# Accessibility), then esc, RUNS times. By the app's clock: from the key's AppKit stamp to the
# display tick after the overview's first frame (`overview`), and after the first frame in which
# every card has its tab's screen (`overview-filled`). Fails when a filled overview took 100 ms or
# more at the median.
source "$(dirname "$0")/lib.sh"
runs=5
tabs=50
while [ $# -gt 0 ]; do
  case $1 in
  --runs) runs=$2 && shift 2 ;;
  --tabs) tabs=$2 && shift 2 ;;
  *) echo "usage: overview.sh [--runs N] [--tabs N]" >&2 && exit 2 ;;
  esac
done
perf_screen_gate
perf_init overview

perf_server
made=0
for s in $(seq 10); do
  sid=$("$LEON" new "s$s")
  made=$((made + 1))
  while [ "$made" -lt $((tabs * s / 10)) ]; do
    "$LEON" run -s "$sid" -- sh -c 'ls -l /; seq 1 40; exec sh' >/dev/null
    made=$((made + 1))
  done
done
perf_app "$RUN/frames.log"
sleep 2
keys() { osascript -e "tell application \"System Events\" to $1"; }
for i in $(seq "$runs"); do
  perf_load_gate
  keys "set frontmost of (first process whose unix id is $APP_PID) to true" >/dev/null
  sleep 0.5
  before=$(grep -c '^overview-filled ' "$RUN/frames.log" || true)
  keys 'keystroke "\\" using {command down, shift down}'
  for _ in $(seq 50); do
    [ "$(grep -c '^overview-filled ' "$RUN/frames.log" || true)" -gt "$before" ] && break
    sleep 0.1
  done
  keys 'key code 53' # esc
  sleep 1
done
perf_note "end"
awk '/^overview / { printf "open %.1f\n", ($3 - $2) / 1e6 } /^overview-filled / { printf "filled %.1f\n", ($3 - $2) / 1e6 }' \
  "$RUN/frames.log" >"$RUN/results.txt"
median() { sort -n | awk '{ v[NR] = $1 } END { if (!NR) print "none"; else if (NR % 2) print v[(NR + 1) / 2]; else printf "%.2f\n", (v[NR / 2] + v[NR / 2 + 1]) / 2 }'; }
open=$(awk '$1 == "open" { print $2 }' "$RUN/results.txt" | median)
filled=$(awk '$1 == "filled" { print $2 }' "$RUN/results.txt" | median)
n=$(grep -c '^filled' "$RUN/results.txt" || true)
echo "overview, $tabs tabs, $n of $runs runs: first frame p50 $open ms, every card filled p50 $filled ms (load $(load1); files in $RUN)"
# an open that never filled is a failure, not a sample left out
[ "$n" -eq "$runs" ] || { echo "overview: FAIL, only $n of $runs opens filled every card" >&2; exit 1; }
awk -v f="$filled" 'BEGIN { exit !(f < 100) }' || { echo "overview: FAIL, filled took $filled ms (limit 100)" >&2; exit 1; }
