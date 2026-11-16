"""logtally: summarize an access log (METHOD PATH STATUS DURATIONms per line)."""
import argparse
import sys
import time
from collections import Counter

DIM, BOLD, RED, GREEN, YELLOW, RESET = "\033[2m", "\033[1m", "\033[31m", "\033[32m", "\033[33m", "\033[0m"


def parse(line):
    parts = line.split()
    if parts and parts[0].count(":") == 2:  # a leading HH:MM:SS, as dev servers print
        parts = parts[1:]
    if len(parts) < 4 or not parts[2].isdigit() or not parts[3].endswith("ms"):
        return None
    try:
        return parts[0], parts[1].split("?")[0], int(parts[2]), float(parts[3][:-2])
    except ValueError:  # "badms": not a duration
        return None


def percentile(sorted_values, p):
    if not sorted_values:
        return 0.0
    k = (len(sorted_values) - 1) * p / 100
    lo = int(k)
    hi = min(lo + 1, len(sorted_values) - 1)
    return sorted_values[lo] + (sorted_values[hi] - sorted_values[lo]) * (k - lo)


def is_error(status, include_4xx=False):
    return status >= 500 or (include_4xx and 400 <= status < 500 and status != 404)


def tally(lines, include_4xx=False):
    paths, statuses, times, skipped = Counter(), Counter(), [], 0
    errors = 0
    for line in lines:
        rec = parse(line)
        if rec is None:
            skipped += 1
            continue
        _, path, status, ms = rec
        paths[path] += 1
        statuses[status // 100] += 1
        times.append(ms)
        errors += is_error(status, include_4xx)
    times.sort()
    n = len(times)
    return {
        "requests": n,
        "skipped": skipped,
        "error_rate": errors / n if n else 0.0,
        "p50": percentile(times, 50),
        "p95": percentile(times, 95),
        "p99": percentile(times, 99),
        "statuses": dict(statuses),
        "top": paths.most_common(),
    }


def render(t, top=5, color=True):
    c = (lambda code, s: f"{code}{s}{RESET}") if color else (lambda code, s: s)
    rate = t["error_rate"] * 100
    out = [
        f"{c(BOLD, 'requests')}  {t['requests']:,}   "
        f"{c(BOLD, 'errors')}  {c(RED if rate >= 1 else GREEN, f'{rate:.2f}%')}   "
        f"{c(BOLD, 'p50')} {t['p50']:.0f}ms  {c(BOLD, 'p95')} {t['p95']:.0f}ms  {c(BOLD, 'p99')} {t['p99']:.0f}ms",
        "  ".join(f"{k}xx {c(DIM, format(v, ','))}" for k, v in sorted(t["statuses"].items())),
        "",
    ]
    width = max((len(p) for p, _ in t["top"][:top]), default=4)
    peak = t["top"][0][1] if t["top"] else 1
    for path, count in t["top"][:top]:
        bar = "█" * max(1, round(24 * count / peak))
        out.append(f"{path:<{width}}  {c(YELLOW, bar)} {count:,}")
    return "\n".join(out)


def follow(path, top):
    lines, partial = [], ""
    with open(path) as f:
        while True:
            chunk = partial + f.read()
            *done, partial = chunk.split("\n")  # a line still being written waits for the next read
            lines.extend(done)
            sys.stdout.write("\033[H\033[2J" + time.strftime("%H:%M:%S  ") + path + "\n\n")
            sys.stdout.write(render(tally(lines), top) + "\n")
            sys.stdout.flush()
            time.sleep(1)


def main(argv=None):
    ap = argparse.ArgumentParser(prog="logtally", description=__doc__)
    ap.add_argument("log", help="the access log, or - for stdin")
    ap.add_argument("--top", type=int, default=5, help="how many paths to show")
    ap.add_argument("--follow", action="store_true", help="keep reading as the log grows")
    ap.add_argument("--errors", choices=["5xx", "4xx"], default="5xx",
                    help="what counts as an error: 5xx only, or 4xx too (not 404)")
    args = ap.parse_args(argv)
    if args.follow:
        return follow(args.log, args.top)
    f = sys.stdin if args.log == "-" else open(args.log)
    print(render(tally(f, args.errors == "4xx"), args.top, sys.stdout.isatty()))


if __name__ == "__main__":
    main()
