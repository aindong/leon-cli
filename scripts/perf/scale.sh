#!/usr/bin/env bash
# scale.sh: many panes, sessions and tabs (docs/performance-app.md).
#   scripts/perf/scale.sh [--secs 20]
# 1. A tab of 5 x 5 panes, then one of 10 x 10, each pane printing a line every 100 ms (perl, a few
#    MB a process): the time of every split (does the 100th cost more than the 1st?), memory, and
#    the app's frames over SECS (frame build time: does it grow faster than the pane count?).
# 2. 50 sessions, then 10 tabs in one session: the time of each create, and memory after.
source "$(dirname "$0")/lib.sh"
secs=20
[ "${1:-}" = --secs ] && secs=$2
perf_screen_gate
perf_init scale
ms_since() { echo $((($(uptime_ns) - $1) / 1000000)); }
perf_server
tick='exec perl -e '"'"'$|=1; while (1) { print "line ", $i++, "\n"; select(undef, undef, undef, 0.1) }'"'"
APP_PID=
for side in 5 10; do
  perf_load_gate
  perf_note "scale grid $side"
  [ -n "$APP_PID" ] && { perf_stop "$APP_PID"; }
  [ -n "${sid:-}" ] && "$LEON" kill "$sid" >/dev/null
  sid=$("$LEON" new "grid-$side")
  : >"$RUN/splits.txt"
  t0=$(uptime_ns)
  cols=$(stack "$sid" "$(first_block "$sid")" "$side" "exec /bin/sh" right)
  [ "$(echo "$cols" | wc -w)" -eq "$side" ] || { echo "scale: the columns were not all made" >&2; exit 1; }
  for c in $cols; do
    stack "$sid" "$c" "$side" "$tick" >/dev/null
  done
  total=$(ms_since "$t0")
  mv "$RUN/splits.txt" "$RUN/grid-$side-splits.txt"
  perf_app "$RUN/grid-$side-frames.log"
  sleep "$secs"
  echo "grid-$side $total $(mem "$SERVER_PID") $(mem "$APP_PID") $(load1)" >>"$RUN/results.txt"
done
perf_stop "$APP_PID"
"$LEON" kill "$sid" >/dev/null
perf_app "$RUN/sessions-frames.log"
perf_load_gate
perf_note "scale sessions"
: >"$RUN/creates.txt"
before="$(mem "$SERVER_PID") $(mem "$APP_PID")"
for i in $(seq 50); do
  t0=$(uptime_ns)
  "$LEON" new "s$i" >/dev/null
  ms_since "$t0" >>"$RUN/creates.txt"
done
sleep 3
echo "sessions-50 $(paste -sd+ "$RUN/creates.txt" | bc) $before $(mem "$SERVER_PID") $(mem "$APP_PID") $(load1)" >>"$RUN/results.txt"
mv "$RUN/creates.txt" "$RUN/sessions-creates.txt"
perf_note "scale tabs"
sid=$("$LEON" new tabs)
before="$(mem "$SERVER_PID") $(mem "$APP_PID")"
for i in $(seq 10); do
  t0=$(uptime_ns)
  "$LEON" -s "$sid" window new >/dev/null
  ms_since "$t0" >>"$RUN/creates.txt"
done
sleep 3
echo "tabs-10 $(paste -sd+ "$RUN/creates.txt" | bc) $before $(mem "$SERVER_PID") $(mem "$APP_PID") $(load1)" >>"$RUN/results.txt"
mv "$RUN/creates.txt" "$RUN/tabs-creates.txt"
perf_note "end"
"$PERF_PY" - "$RUN" <<'PY' | tee "$RUN/summary.txt"
import sys, statistics as st
run = sys.argv[1]
def nums(path):
    return [float(x) for x in open(path).read().split()]
def pct(xs, p):  # nearest rank, as stats.py
    xs = sorted(xs); return xs[max(0, -(-len(xs) * p // 100) - 1)]
for l in open(f"{run}/results.txt"):
    k, *v = l.split()
    if k.startswith("grid"):
        side = int(k.split("-")[1]); n = side * side
        splits = nums(f"{run}/{k}-splits.txt")
        frames = [list(map(int, x.split()[1:])) for x in open(f"{run}/{k}-frames.log") if x.startswith("frame ")]
        frames = frames[len(frames) // 4:]  # past start-up
        build = [(e - s) / 1e6 for s, e, _ in frames]
        span = (frames[-1][2] - frames[0][2]) / 1e9 if len(frames) > 1 else 1
        print(f"{n} panes: created in {float(v[0]) / 1000:.1f} s ({len(splits)} splits: first ten median {st.median(splits[:10]):.0f} ms, last ten {st.median(splits[-10:]):.0f} ms); "
              f"server {int(v[2]) / 1024:.0f} MiB, app {int(v[4]) / 1024:.0f} MiB footprint; "
              f"app {len(frames) / span:.1f} fps, frame build p50 {pct(build, 50):.2f} ms, p99 {pct(build, 99):.2f} ms (load={v[5]})")
    else:
        what = "50 sessions" if k.startswith("sessions") else "10 tabs"
        c = nums(f"{run}/{k.split('-')[0]}-creates.txt")
        print(f"{what}: {float(v[0]):.0f} ms in all (median {st.median(c):.0f} ms each, first {c[0]:.0f}, last {c[-1]:.0f}); "
              f"server footprint {int(v[2]) / 1024:.0f} -> {int(v[6]) / 1024:.0f} MiB, app {int(v[4]) / 1024:.0f} -> {int(v[8]) / 1024:.0f} MiB (load={v[9]})")
PY
