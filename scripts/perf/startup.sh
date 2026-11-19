#!/usr/bin/env bash
# startup.sh: how long leon takes to start (docs/performance-app.md).
#   scripts/perf/startup.sh [--runs N]
# - app, no server: `leon app` with no server running (it starts one, which starts the session's
#   shell), to the first frame that shows the shell's prompt. Every run has a fresh state dir. The
#   binary is in the page cache (no root to purge it), so this is "first launch, warm cache".
# - app, server up: the same with the server and its session already running.
# Both by the app's clock (LEON_FRAME_LOG's first frame with text: "$ " after the sentinel-bar
# prompt) and from outside (winframes: the window appearing, the first frame showing the prompt bar).
# - `leon run --wait -- true`: a client's whole round trip, process start included.
# - `leon server start`: from the command to its return with the server answering.
source "$(dirname "$0")/lib.sh"
runs=10
[ "${1:-}" = --runs ] && runs=$2
perf_screen_gate
perf_init startup
ms() { awk -v a="$1" -v b="$2" 'BEGIN { printf "%.1f", (b - a) / 1e6 }'; }

app_run() { # app_run KIND I: one launch, appending "KIND inapp_ms window_ms prompt_ms" to results
  local log="$RUN/$1-$2-frames.log" t0 w p
  t0=$(uptime_ns)
  LEON_FRAME_LOG="$log" "$LEON" app 2>>"$RUN/app-stderr.txt" &
  APP_PID=$!
  PERF_PIDS+=("$APP_PID")
  "$WINFRAMES" "$APP_PID" watch 10 1 >"$RUN/$1-$2-watch.txt" || true
  w=$(awk '/^window / {print $2; exit}' "$RUN/$1-$2-watch.txt")
  p=$(awk '/^frame / { if (!seen++ && $3 >= 1) { print -1; exit } if ($3 >= 1) { print $2; exit } }' "$RUN/$1-$2-watch.txt")
  [ "${p:-0}" = -1 ] && p=$t0 # drawn before the capture began: no outside number for this run
  echo "$1 $(ms "$t0" "$(awk '/^text / {print $2; exit}' "$log")") $(ms "$t0" "${w:-0}") $(ms "$t0" "${p:-0}")" >>"$RUN/results.txt"
  perf_stop "$APP_PID"
}

for i in $(seq "$runs"); do
  perf_load_gate
  export LEON_STATE_DIR="$PERF_DIR/state-$i"
  mkdir -m 700 "$LEON_STATE_DIR"
  app_run cold "$i"
  "$LEON" --autostart=false server stop >/dev/null 2>&1 || true
done
perf_server
warm=$("$LEON" new warm)
for i in $(seq "$runs"); do
  perf_load_gate
  app_run warm "$i"
done
for i in $(seq "$runs"); do
  t0=$(uptime_ns)
  "$LEON" run -s "$warm" --wait --output=false -- true >/dev/null
  echo "run $(ms "$t0" "$(uptime_ns)")" >>"$RUN/results.txt"
done
"$LEON" --autostart=false server stop >/dev/null
for i in $(seq "$runs"); do
  export LEON_STATE_DIR="$PERF_DIR/server-$i"
  mkdir -m 700 "$LEON_STATE_DIR"
  t0=$(uptime_ns)
  "$LEON" server start
  echo "server $(ms "$t0" "$(uptime_ns)")" >>"$RUN/results.txt"
  "$LEON" --autostart=false server stop >/dev/null
done
# the stamps themselves cost a Python start each; that floor is subtracted from nothing, only said
t0=$(uptime_ns)
echo "stamp $(ms "$t0" "$(uptime_ns)")" >>"$RUN/results.txt"
perf_note "end"
"$PERF_PY" - "$RUN/results.txt" <<'PY' | tee "$RUN/summary.txt"
import sys, statistics as st
rows = [l.split() for l in open(sys.argv[1])]
def med(kind, col, what):
    xs = [float(r[col]) for r in rows if r[0] == kind and len(r) > col and float(r[col]) > 0]
    if xs:
        print(f"{what}: median {st.median(xs):.1f} ms, min {min(xs):.1f}, max {max(xs):.1f} (n={len(xs)})")
for kind, name in (("cold", "app, no server running"), ("warm", "app, server up")):
    med(kind, 1, f"{name}: launch -> first frame with the prompt (in app)")
    med(kind, 2, f"{name}: launch -> window on screen (outside)")
    med(kind, 3, f"{name}: launch -> prompt bar composited (outside; runs where it was drawn before the capture began are left out)")
med("run", 1, "leon run --wait -- true")
med("server", 1, "leon server start")
med("stamp", 1, "one clock stamp (included in each number above)")
PY
