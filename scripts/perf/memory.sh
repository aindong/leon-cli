#!/usr/bin/env bash
# memory.sh: RSS and phys_footprint (KiB) of the server and the app (docs/performance.md).
#   scripts/perf/memory.sh [--gb 1]
# 1. idle, one session with one shell pane, the app showing it
# 2. 1, 10 and 50 more panes (each a new tab running /bin/sh: the server holds every one, the app
#    draws the active tab)
# 3. scrollback: 5 panes each printing 10,000 lines of 67 bytes (leon keeps 10,000 lines a pane)
# 4. a flood of GB gigabytes (`cat` of the 100 MB flood file, 10 times a GB) in the shown pane:
#    sampled every 2 s during it, then 5, 30 and 60 s after it ends
# Each line of samples.txt: "<step> <server rss> <server footprint> <app rss> <app footprint> <load>".
source "$(dirname "$0")/lib.sh"
gb=1
[ "${1:-}" = --gb ] && gb=$2
perf_screen_gate
perf_init memory
flood="$PERF_ROOT/target/perf/flood-100.txt"
[ -s "$flood" ] || "$PERF_PY" "$PERF_ROOT/scripts/perf/mkflood.py" "$flood" 100
perf_server
sid=$("$LEON" new mem)
perf_app "$RUN/frames.log"
sample() { echo "$1 $(mem "$SERVER_PID") $(mem "$APP_PID") $(load1)" | tee -a "$RUN/samples.txt"; }
settle() { sleep 3; }

perf_load_gate
settle
sample idle
added=0
for target in 1 10 50; do
  while [ "$added" -lt "$target" ]; do
    "$LEON" -s "$sid" window new >/dev/null
    added=$((added + 1))
  done
  settle
  sample "panes+$target"
done

perf_load_gate
sid2=$("$LEON" new scroll)
sample "scroll-before"
for i in 1 2 3 4 5; do
  "$LEON" run -s "$sid2" -- "$PERF_PY" "$PERF_ROOT/scripts/perf/agent.py" fill 10000 >/dev/null
done
settle
sample "scroll-5x10k"
"$LEON" kill "$sid2" >/dev/null

perf_load_gate
"$LEON" kill "$sid" >/dev/null
sid=$("$LEON" new flood)
settle
sample "flood-before"
done_file="$RUN/flood-done"
times=$((gb * 10))
"$LEON" split -s "$sid" -- sh -c "for i in \$(seq $times); do cat '$flood'; done; touch '$done_file'; exec sleep 100000" >/dev/null
deadline=$(($(date +%s) + 60 * gb * 10)) # a minute per 100 MB is far slower than any run seen
while [ ! -e "$done_file" ]; do
  kill -0 "$SERVER_PID" 2>/dev/null || { echo "memory: the server exited during the flood" >&2; exit 1; }
  [ "$(date +%s)" -lt "$deadline" ] || { echo "memory: the flood did not finish in time" >&2; exit 1; }
  sample "flood-during"
  sleep 2
done
for after in 5 30 60; do
  sleep $((after - ${last:-0}))
  last=$after
  sample "flood-after-${after}s"
done
perf_note "end"
"$PERF_PY" - "$RUN/samples.txt" <<'PY' | tee "$RUN/summary.txt"
import sys
rows = {}
during = []
for l in open(sys.argv[1]):
    step, *v = l.split()
    v = [int(x) for x in v[:4]]
    if step == "flood-during":
        during.append(v)
    rows[step] = v
mib = lambda k: f"{k / 1024:.1f} MiB"
def show(step, label):
    s, sf, a, af = rows[step]
    print(f"{label}: server rss {mib(s)}, footprint {mib(sf)}; app rss {mib(a)}, footprint {mib(af)}")
show("idle", "idle, 1 pane")
for n in (1, 10, 50):
    show(f"panes+{n}", f"{n} more panes")
per = [(rows["panes+50"][i] - rows["idle"][i]) / 50 for i in range(4)]
print(f"per extra pane (50): server rss {per[0]:.0f} KiB, footprint {per[1]:.0f} KiB; app rss {per[2]:.0f} KiB, footprint {per[3]:.0f} KiB")
d = [(rows["scroll-5x10k"][i] - rows["scroll-before"][i]) / 5 for i in range(4)]
print(f"per 10,000 lines of scrollback: server footprint {d[1] / 1024:.1f} MiB (rss {d[0] / 1024:.1f}), app footprint {d[3] / 1024:.1f} MiB")
show("flood-before", "before the flood")
if during:
    print(f"during the flood (peak of {len(during)} samples): server footprint {mib(max(v[1] for v in during))}, app footprint {mib(max(v[3] for v in during))}")
for a in (5, 30, 60):
    show(f"flood-after-{a}s", f"{a} s after the flood")
PY
