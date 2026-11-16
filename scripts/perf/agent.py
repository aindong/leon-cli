"""A pane's workload for the perf scripts.

python3 agent.py busy             bursts of ANSI text forever: 20 lines every 20 ms (about 1000 lines/s)
python3 agent.py soak [SECONDS]   an agent: fills the scrollback first, then every 0.5 s a burst of 5
                                  to 60 lines, and every 5 s an OSC 7501 report (working, blocked,
                                  done in turn); exits after SECONDS (default: random 60 to 300)
python3 agent.py fill LINES       print LINES numbered lines, then wait (scrollback cost)
"""
import random
import sys
import time

mode = sys.argv[1] if len(sys.argv) > 1 else "busy"
rng = random.Random()
words = "reading file running tests edit apply patch thinking tool call ok passed failed diff".split()
out = sys.stdout


def line(n):
    parts = [rng.choice(words) for _ in range(rng.randint(4, 12))]
    c = rng.choice([31, 32, 33, 34, 35, 36, 90])
    return f"\033[{c}m{n:>6}\033[0m \033[1m{parts[0]}\033[0m {' '.join(parts[1:])}\n"


n = 0
if mode == "fill":
    for n in range(int(sys.argv[2])):
        out.write(f"{n} {'x' * 60}\n")
    out.flush()
    while True:
        time.sleep(3600)
elif mode == "busy":
    while True:
        out.write("".join(line(n + i) for i in range(20)))
        out.flush()
        n += 20
        time.sleep(0.02)
else:
    life = float(sys.argv[2]) if len(sys.argv) > 2 else rng.uniform(60, 300)
    # the scrollback starts full (10,000 lines is leon's cap), so its growth is not mistaken for a leak
    out.write("".join(line(i) for i in range(10_000)))
    end, states, k = time.time() + life, ["working", "blocked:kind=question", "done"], 0
    next_status = 0.0
    while time.time() < end:
        if time.time() >= next_status:
            out.write(f"\033]7501;state={states[k % 3]}\007")
            k, next_status = k + 1, time.time() + 5
        out.write("".join(line(n + i) for i in range(rng.randint(5, 60))))
        out.flush()
        n += 60
        time.sleep(0.5)
