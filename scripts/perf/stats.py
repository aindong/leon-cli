"""Summaries of the perf logs. Every time in them is the boot clock in nanoseconds.

python3 stats.py keylat WINFRAMES_OUT FRAME_LOG
python3 stats.py flood WATCH_OUT FRAME_LOG MARKS [PANES]   (MARKS: "begin NS" and "end NS" lines,
    the first begin and last end count; the screen is current once PANES sentinel bars show)
python3 stats.py selftest
"""
import sys


def pct(xs, p):
    xs = sorted(xs)
    return xs[min(len(xs) - 1, int(len(xs) * p / 100))] if xs else float("nan")


def ms(ns):
    return ns / 1e6


def lines(path, tag):
    with open(path) as f:
        return [list(map(int, l.split()[1:])) for l in f if l.startswith(tag + " ")]


def summary(name, xs):
    if not xs:
        return f"{name}: no samples"
    return f"{name}: p50 {pct(xs, 50):.2f} ms, p99 {pct(xs, 99):.2f} ms, max {max(xs):.2f} ms (n={len(xs)})"


def keylat(ext, log):
    out = []
    keys = lines(ext, "key")
    missed = len(lines(ext, "missed"))
    out.append(summary("outside (key posted -> frame composited with its echo)", [ms(s - p) for _, p, s in keys]) + f", missed {missed}")
    app = lines(log, "key")
    with open(log) as f:
        text = f.read()
    overlaps, bad = text.count("\noverlap ") + text.startswith("overlap "), text.count("badclock ")
    out.append(summary("in app (event stamp -> key_down)", [ms(h - e) for e, h, _, _ in app]))
    out.append(summary("in app (event stamp -> echo pane painted)", [ms(p - e) for e, _, p, _ in app]))
    out.append(summary("in app (event stamp -> next display tick)", [ms(t - e) for e, _, _, t in app]) + f", overlaps {overlaps}, bad clock {bad}")
    return out


def flood(watch, log, marks, panes=1):
    stamps = {"begin": [], "end": []}
    with open(marks) as f:
        for l in f:
            k, v = l.split()
            stamps[k].append(int(v))
    begin, end, panes = min(stamps["begin"]), max(stamps["end"]), int(panes)
    period = lines(watch, "period")[0][0]
    frames = lines(watch, "frame")  # time, sentinel bars, changed (the capture repeats unchanged frames)
    current = next((t for t, s, _ in frames if t > end and s >= panes), None)
    shown = [t for t, _, new in frames if new and begin <= t <= (current or end)]
    gaps = [b - a for a, b in zip(shown, shown[1:])]
    span = (current or end) - begin
    out = [f"flood: writer {ms(end - begin):.0f} ms; screen current {ms(current - end):.1f} ms after the writer's last byte"
           if current else f"flood: writer {ms(end - begin):.0f} ms; the sentinel was never shown (FAIL)"]
    out.append(f"outside: {len(shown)} frames in {ms(span):.0f} ms = {len(shown) / (span / 1e9):.1f} fps (display {1e9 / period:.0f} Hz); "
               f"frames not shown (gaps over 1.5 periods) {sum(int(g / period - 0.5) for g in gaps if g > 1.5 * period)}; "
               + summary("gap", [ms(g) for g in gaps]))
    fl = [t for t, new in lines(watch, "floor") if new and begin <= t <= (current or end)]
    if fl:
        out.append(f"capture floor: an animated window of the tool's own got {len(fl) / (span / 1e9):.1f} fps over the same span")
    app = [f for f in lines(log, "frame") if begin <= f[2] <= (current or end) + 100_000_000]
    out.append(summary("in app frame build (render start -> last pane painted)", [ms(e - s) for s, e, _ in app]))
    ticks = [t for _, _, t in app]
    out.append(f"in app: {len(app)} frames = {len(app) / (span / 1e9):.1f} fps; " + summary("tick gap", [ms(b - a) for a, b in zip(ticks, ticks[1:])]))
    return out


def selftest():
    import os
    import tempfile
    d = tempfile.mkdtemp()
    w, l, mk = (os.path.join(d, n) for n in ("w", "l", "m"))
    p = 16_666_666
    with open(w, "w") as f:
        f.write(f"period {p}\n" + "".join(f"frame {i * p} 0 1\nframe {i * p + 9} 0 0\n" for i in range(10) if i != 5) + f"frame {12 * p} 1 1\n")
    with open(l, "w") as f:
        f.write("".join(f"frame {i * p} {i * p + 2_000_000} {i * p + 3_000_000}\n" for i in range(10)))
        f.write("key 0 1000000 3000000 5000000\noverlap 9\n")
    with open(mk, "w") as f:
        f.write(f"begin 0\nbegin 5\nend 7\nend {10 * p}\n")
    out = flood(w, l, mk)
    assert "sentinel was never shown" in flood(w, l, mk, 2)[0]
    assert "screen current 33.3 ms" in out[0], out
    assert "frames not shown (gaps over 1.5 periods) 3" in out[1], out  # 4->6 (1), 9->12 (2)
    assert "p50 2.00 ms" in out[2], out
    k = os.path.join(d, "k")
    with open(k, "w") as f:
        f.write("key 1 0 8000000\nkey 2 100 4000100\nmissed 3 9\n")
    out = keylat(k, l)
    assert "p50 8.00 ms" in out[0] and "missed 1" in out[0], out
    assert "p50 3.00 ms" in out[2] and "overlaps 1" in out[3], out
    print("stats selftest ok")


if __name__ == "__main__":
    cmd = sys.argv[1]
    if cmd == "selftest":
        selftest()
    else:
        print("\n".join({"keylat": keylat, "flood": flood}[cmd](*sys.argv[2:])))
