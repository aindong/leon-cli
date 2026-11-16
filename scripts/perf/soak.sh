#!/usr/bin/env bash
# soak.sh: a long agent workload, sampling memory to find leaks (docs/performance.md).
#   scripts/perf/soak.sh [MINUTES]   (default 120)
# 4 sessions of 3 panes, each pane an agent (scripts/perf/agent.py soak: a full scrollback first,
# then ANSI bursts and OSC 7501 working/blocked/done every 5 s) that exits after 1 to 5 minutes
# and starts again. Every minute a session is created (2 agents) and the one before is killed, and
# a `leon attach` client attaches for 20 s and detaches. The app shows the first session, its
# unfocused panes faded, throughout.
# Every minute: RSS and footprint of the server and the app, and the load, into soak.csv;
# `leaks` and `heap` on both at the start (after 5 minutes), the middle and the end. A sample in
# the minute after a `leaks` pause is marked. Then soak.svg and the leak verdict (chart.py).
source "$(dirname "$0")/lib.sh"
minutes=${1:-120}
perf_screen_gate
perf_init soak
perf_server
agent="while :; do '$PERF_PY' '$PERF_ROOT/scripts/perf/agent.py' soak; sleep 1; done"
first=
for s in 1 2 3 4; do
  sid=$("$LEON" new "agents-$s")
  first=${first:-$sid}
  stack "$sid" "$(first_block "$sid")" 3 "$agent" >/dev/null
done
perf_app "$RUN/frames.log"
"$LEON" focus -s "$first" "$(first_block "$first")" >/dev/null 2>&1 || true
echo "minute,server_rss_kib,server_footprint_kib,app_rss_kib,app_footprint_kib,load,after_leaks" >"$RUN/soak.csv"

probe() { # probe WHEN: leaks and heap on both processes
  local who pid
  for who in server app; do
    pid=$SERVER_PID
    [ "$who" = app ] && pid=$APP_PID
    leaks "$pid" >"$RUN/leaks-$1-$who.txt" 2>&1 || true
    heap -s "$pid" 2>/dev/null | head -60 >"$RUN/heap-$1-$who.txt" || true
  done
  perf_note "leaks $1: $(grep -h 'leaks for' "$RUN"/leaks-"$1"-*.txt | tr '\n' ' ')"
}

churn=
after=0
for m in $(seq 0 "$minutes"); do
  t0=$(date +%s)
  if ! kill -0 "$APP_PID" 2>/dev/null || ! kill -0 "$SERVER_PID" 2>/dev/null; then
    perf_note "minute $m: a process died"
    echo "soak: the app or the server exited at minute $m" >&2
    exit 1
  fi
  echo "$m,$(mem "$SERVER_PID" | tr ' ' ','),$(mem "$APP_PID" | tr ' ' ','),$(load1),$after" >>"$RUN/soak.csv"
  after=0
  # the end first, so a short soak still ends (and probes) on time
  if [ "$m" = "$minutes" ]; then
    probe end
    break
  elif [ "$m" = $((minutes / 2)) ]; then
    probe middle
    after=1
  elif [ "$m" = 5 ]; then
    probe start
    after=1
  fi
  # a session created and the last one killed
  new=$("$LEON" new "churn-$m")
  stack "$new" "$(first_block "$new")" 2 "exec '$PERF_PY' '$PERF_ROOT/scripts/perf/agent.py' soak 90" >/dev/null
  [ -n "$churn" ] && { "$LEON" kill "$churn" >/dev/null || true; }
  churn=$new
  # an attach client for 20 s (script(1) gives it a terminal); SIGTERM detaches it
  script -q /dev/null "$LEON" attach -s "$new" >/dev/null 2>&1 &
  attach=$!
  PERF_PIDS+=("$attach")
  sleep 20
  pkill -TERM -P "$attach" 2>/dev/null || true # leon attach itself: script(1) doesn't pass SIGTERM on
  kill "$attach" 2>/dev/null || true
  wait "$attach" 2>/dev/null || true
  sleep $((60 - ($(date +%s) - t0) > 0 ? 60 - ($(date +%s) - t0) : 0))
done
perf_note "end"
"$PERF_PY" "$PERF_ROOT/scripts/perf/chart.py" "$RUN/soak.csv" "$RUN/soak.svg" | tee "$RUN/summary.txt"
