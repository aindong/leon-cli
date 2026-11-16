#!/usr/bin/env bash
# external.sh: the same measurements from outside, for any terminal (docs/performance.md).
#   scripts/perf/external.sh MEASURE [--runs N] -- LAUNCH...
# LAUNCH starts a terminal window running one program, given as `{}` (a script path); its pid
# must be the process that owns the window. scripts/perf/leon-term.sh does that for leon:
#   scripts/perf/external.sh keylat -- scripts/perf/leon-term.sh {}
# MEASURE:
#   startup  launch to the window on screen, and to the prompt drawn (sh, sentinel-bar PS1)
#   keylat   key to pixel: scripts/perf/echo.py, winframes keys (no in-app clock: outside only)
#   flood    `cat` of the 100 MB flood file: frames shown, screen current after the last byte
#   idle     memory (footprint) 10 s after start and CPU over 60 s, one idle shell
# The terminal runs with HOME, XDG_CONFIG_HOME and the LEON_* dirs in a private temp dir; give
# LAUNCH whatever flags make it skip its own config. Only the launched pid (and pids it lists in
# $PERF_DIR/extra-pids, such as leon's server) is measured and killed.
source "$(dirname "$0")/lib.sh"
measure=$1
shift
runs=5
[ "${1:-}" = --runs ] && { runs=$2; shift 2; }
[ "${1:-}" = -- ] && shift
[ $# -gt 0 ] || { echo "usage: external.sh startup|keylat|flood|idle [--runs N] -- LAUNCH... ({} is the program)" >&2; exit 2; }
perf_screen_gate
perf_init "external-$measure"
export XDG_CONFIG_HOME="$PERF_DIR/home/.config" PERF_DIR
flood="$PERF_ROOT/target/perf/flood-100.txt"
[ -s "$flood" ] || "$PERF_PY" "$PERF_ROOT/scripts/perf/mkflood.py" "$flood" 100

# launch PROGRAM_SCRIPT: start the terminal running it; sets TERM_PID
launch() {
  local args=() a
  for a in "${LAUNCH[@]}"; do args+=("${a//\{\}/$1}"); done
  "${args[@]}" >>"$RUN/terminal-output.txt" 2>&1 &
  TERM_PID=$!
  PERF_PIDS+=("$TERM_PID")
}
quit() {
  local p
  perf_stop "$TERM_PID"
  if [ -s "$PERF_DIR/extra-pids" ]; then
    while read -r p; do kill "$p" 2>/dev/null || true; done <"$PERF_DIR/extra-pids"
    : >"$PERF_DIR/extra-pids"
  fi
  sleep 1
}
program() { # program NAME BODY: a program file for {}
  printf '#!/bin/sh\n%s\n' "$2" >"$PERF_DIR/$1"
  chmod +x "$PERF_DIR/$1"
  echo "$PERF_DIR/$1"
}
LAUNCH=("$@")
stamp="import time,sys; print(sys.argv[1], time.clock_gettime_ns(time.CLOCK_UPTIME_RAW))"

case "$measure" in
  startup)
    prog=$(program shell "exec /bin/sh -i")
    for i in $(seq "$runs"); do
      perf_load_gate
      t0=$(uptime_ns)
      launch "$prog"
      "$WINFRAMES" "$TERM_PID" watch 15 1 >"$RUN/$i-watch.txt" || true
      quit
      w=$(awk '/^window / {print $2; exit}' "$RUN/$i-watch.txt")
      p=$(awk '/^frame / { if (!seen++ && $3 >= 1) { print -1; exit } if ($3 >= 1) { print $2; exit } }' "$RUN/$i-watch.txt")
      bound=0
      if [ "${p:-0}" = -1 ]; then # drawn before the capture began: the capture's start is a bound
        p=$(awk '/^start / {print $2; exit}' "$RUN/$i-watch.txt") bound=1
      fi
      echo "$t0 ${w:-0} ${p:-0} $bound" >>"$RUN/results.txt"
    done
    "$PERF_PY" - "$RUN/results.txt" <<'PY' | tee "$RUN/summary.txt"
import sys, statistics as st
rows = [list(map(int, l.split())) for l in open(sys.argv[1])]
for i, what in ((1, "launch -> window on screen"), (2, "launch -> prompt drawn")):
    xs = [(r[i] - r[0]) / 1e6 for r in rows if r[i] > 0]
    bound = sum(r[3] for r in rows) if i == 2 else 0
    note = f"; in {bound} runs the prompt was already drawn when the capture began, so those are upper bounds" if bound else ""
    print(f"{what}: median {st.median(xs):.0f} ms, min {min(xs):.0f}, max {max(xs):.0f} (n={len(xs)}{note})" if xs else f"{what}: never seen")
PY
    ;;
  keylat)
    perf_load_gate
    launch "$(program echo "exec '$PERF_PY' '$PERF_ROOT/scripts/perf/echo.py'")"
    sleep 3
    "$WINFRAMES" "$TERM_PID" keys 200 50 >"$RUN/winframes.txt"
    quit
    : >"$RUN/empty.log"
    "$PERF_PY" "$PERF_ROOT/scripts/perf/stats.py" keylat "$RUN/winframes.txt" "$RUN/empty.log" | head -1 | tee "$RUN/summary.txt"
    ;;
  flood)
    perf_load_gate
    go="$RUN/go"
    launch "$(program flood "while [ ! -e '$go' ]; do sleep 0.02; done
'$PERF_PY' -c '$stamp' begin >>'$RUN/marks.txt'; cat '$flood'; '$PERF_PY' -c '$stamp' end >>'$RUN/marks.txt'
printf '\\033[48;2;255;0;128m%60s\\033[0m\\n' ''; exec sleep 100000")"
    sleep 3
    "$WINFRAMES" floor 200 120 animate >/dev/null &
    floor=$!
    PERF_PIDS+=("$floor")
    sleep 1
    "$WINFRAMES" "$TERM_PID" watch 120 1 "floor=$floor" >"$RUN/watch.txt" &
    watch=$!
    PERF_PIDS+=("$watch")
    for i in $(seq 250); do grep -q '^start ' "$RUN/watch.txt" && break; sleep 0.1; done
    sleep 0.5
    touch "$go"
    wait "$watch" || true
    perf_stop "$floor"
    quit
    : >"$RUN/empty.log"
    "$PERF_PY" "$PERF_ROOT/scripts/perf/stats.py" flood "$RUN/watch.txt" "$RUN/empty.log" "$RUN/marks.txt" 1 | head -3 | tee "$RUN/summary.txt"
    ;;
  idle)
    perf_load_gate
    launch "$(program shell "exec /bin/sh -i")"
    sleep 10
    pids="$TERM_PID $(cat "$PERF_DIR/extra-pids" 2>/dev/null || true)"
    for p in $pids; do echo "pid $p $(ps -o comm= -p "$p" | xargs basename) rss/footprint KiB: $(mem "$p")"; done | tee "$RUN/summary.txt"
    c0=0 c1=0
    for p in $pids; do c0=$(echo "$c0 + $(cpu_s "$p")" | bc); done
    sleep 60
    for p in $pids; do c1=$(echo "$c1 + $(cpu_s "$p")" | bc); done
    echo "idle CPU over 60 s: $(echo "scale=2; 100 * ($c1 - $c0) / 60" | bc)% of a core (load=$(load1))" | tee -a "$RUN/summary.txt"
    quit
    ;;
  *) echo "unknown measure $measure" >&2; exit 2 ;;
esac
perf_note "end"
