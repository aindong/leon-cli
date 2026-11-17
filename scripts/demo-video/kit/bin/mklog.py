"""mklog.py OUT SIZE_MB: a big coloured application log (levels, services, request lines), seeded."""
import os
import random
import sys

out, size = sys.argv[1], int(sys.argv[2]) * 1_000_000
rng = random.Random(7501)
RESET, DIM = "\033[0m", "\033[2m"
LEVELS = [("INFO ", "\033[32m", 80), ("DEBUG", "\033[34m", 12), ("WARN ", "\033[33m", 6), ("ERROR", "\033[1;31m", 2)]
SERVICES = ["api", "worker", "search", "auth", "billing", "export", "cache", "gateway"]
COLORS = ["\033[36m", "\033[35m", "\033[94m", "\033[96m", "\033[95m", "\033[93m", "\033[92m", "\033[91m"]
MESSAGES = [
    # short enough (with the prefix, under 50 columns) to fit a narrow pane without wrapping
    lambda: f"GET /books/{rng.randint(1, 9999)} {rng.choice([200, 200, 200, 304, 404])} {rng.lognormvariate(2.3, 0.6):.0f}ms",
    lambda: f"POST /orders {rng.choice([201, 201, 409, 422])} {rng.lognormvariate(3.0, 0.5):.0f}ms",
    lambda: f"job {rng.randint(100, 9999)}: {rng.randint(10, 9000)} rows",
    lambda: f"cache {'hit' if rng.random() < 0.8 else 'miss'} user:{rng.randint(1, 50000)}",
    lambda: f"queue {rng.randint(0, 400)}, {rng.randint(1, 16)} busy",
    lambda: f"retry {rng.randint(1, 3)}/3 in {rng.choice([100, 200, 400])}ms",
]
written, t = 0, 9 * 3600_000  # ms since midnight, from 09:00
tmp = out + ".partial"  # renamed when complete: a cut-off run leaves no file that looks done
with open(tmp, "w") as f:
    while written < size:
        t += rng.randint(0, 30)
        h, m, s, ms = t // 3600_000 % 24, t // 60_000 % 60, t // 1000 % 60, t % 1000
        name, color, _ = rng.choices(LEVELS, [w for *_, w in LEVELS])[0]
        i = rng.randrange(len(SERVICES))
        line = (f"{DIM}{h:02}:{m:02}:{s:02}{RESET} {color}{name}{RESET} "
                f"{COLORS[i]}{SERVICES[i]:<7}{RESET} {rng.choice(MESSAGES)()}\n")
        f.write(line)
        written += len(line)
os.replace(tmp, out)
