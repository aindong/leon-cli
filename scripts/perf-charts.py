#!/usr/bin/env python3
"""perf-charts.py [--check]: draw the benchmark charts in docs/images/perf/ from the data in docs/perf/.

    python3 scripts/perf-charts.py           write every chart
    python3 scripts/perf-charts.py --check   exit 1 naming each chart that differs from its data

Each chart is one CSV in docs/perf/ and one SVG of the same name, made from that file alone, so a
number changed in the data and not in the chart fails the check (crates/leon/tests/perf_charts.rs).
Python's standard library only. The SVGs carry their own background, light or dark with the reader's
system, so they read on GitHub and the website in either theme; axes start at zero and name a unit.
"""

import csv
import math
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
DATA = ROOT / "docs" / "perf"
OUT = ROOT / "docs" / "images" / "perf"
W = 720
PAD = 24
FONT = "-apple-system, BlinkMacSystemFont, 'Segoe UI', Helvetica, Arial, sans-serif"
# the dataviz reference palette: blue and orange for two series, a grey for "before" beside a blue
STYLE = """
  .bg { fill: #fcfcfb; } .t1 { fill: #0b0b0b; } .t2 { fill: #52514e; }
  .grid { stroke: #e7e6e2; } .axis { stroke: #b9b8b1; } .ref { stroke: #52514e; }
  .c1 { fill: #2a78d6; stroke: #2a78d6; } .c2 { fill: #eb6834; stroke: #eb6834; } .c0 { fill: #8f8d86; stroke: #8f8d86; }
  .ring { stroke: #fcfcfb; }
  @media (prefers-color-scheme: dark) {
    .bg { fill: #1a1a19; } .t1 { fill: #f0efec; } .t2 { fill: #c3c2b7; }
    .grid { stroke: #2f2f2c; } .axis { stroke: #57564f; } .ref { stroke: #c3c2b7; }
    .c1 { fill: #3987e5; stroke: #3987e5; } .c2 { fill: #d95926; stroke: #d95926; } .c0 { fill: #76756f; stroke: #76756f; }
    .ring { stroke: #1a1a19; }
  }
  text { font-family: %s; }
""" % FONT


def rows(name):
    with open(DATA / f"{name}.csv", newline="", encoding="utf-8") as f:
        return list(csv.DictReader(f))


def esc(s):
    return s.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;").replace('"', "&quot;")


def f(v):
    """A coordinate, the same text on every Python."""
    return f"{v:.1f}"


def text(x, y, s, cls="t1", size=13, anchor="start", weight=None):
    w = f' font-weight="{weight}"' if weight else ""
    return f'<text x="{f(x)}" y="{f(y)}" class="{cls}" font-size="{size}" text-anchor="{anchor}"{w}>{esc(s)}</text>'


def nice(top):
    """The axis's end and step: a round step (1, 2, 2.5 or 5 times a power of ten), about five of them."""
    k = 10 ** math.floor(math.log10(top / 6))
    step = next(m * k for m in (1, 2, 2.5, 5, 10) if top / (m * k) <= 6)
    return math.ceil(top / step - 1e-9) * step, step


def ticks(end, step):
    return [i * step for i in range(round(end / step) + 1)]


def label(v):
    return f"{v:.3f}".rstrip("0").rstrip(".")


def frame(title, sub, desc, height, body):
    head = [
        f'<svg xmlns="http://www.w3.org/2000/svg" width="{W}" height="{height}" viewBox="0 0 {W} {height}" role="img" aria-labelledby="title desc">',
        f'<title id="title">{esc(title)}</title>',
        f'<desc id="desc">{esc(desc)}</desc>',
        f"<style>{STYLE}</style>",
        f'<rect class="bg" width="{W}" height="{height}" rx="8"/>',
        text(PAD, 32, title, size=16, weight="600"),
    ]
    head += [text(PAD, 52 + 17 * i, line, "t2", 12) for i, line in enumerate(sub)]
    return "\n".join(head + body + ["</svg>"]) + "\n"


def legend(x, y, series):
    out = []
    for name, cls in series:
        out.append(f'<rect x="{f(x)}" y="{f(y - 9)}" width="10" height="10" rx="2" class="{cls}"/>')
        out.append(text(x + 15, y, name, "t2", 12))
        x += 15 + 7 * len(name) + 18
    return out


def xaxis(x0, x1, y_top, y_bottom, end, step, unit):
    out = []
    for v in ticks(end, step):
        x = x0 + (x1 - x0) * v / end
        out.append(f'<line x1="{f(x)}" y1="{f(y_top)}" x2="{f(x)}" y2="{f(y_bottom)}" class="{"axis" if v == 0 else "grid"}" stroke-width="1"/>')
        out.append(text(x, y_bottom + 16, label(v), "t2", 11, "middle"))
    out.append(text((x0 + x1) / 2, y_bottom + 34, unit, "t2", 12, "middle"))
    return out


def ref(x, y_top, y_bottom, name, right_half=False):
    """A dashed reference line, its name beside its top (to its left when the line is far right)."""
    return [
        f'<line x1="{f(x)}" y1="{f(y_top - 6)}" x2="{f(x)}" y2="{f(y_bottom)}" class="ref" stroke-width="1" stroke-dasharray="4 3"/>',
        text(x - 4, y_top - 8, name, "t2", 11, "end") if right_half else text(x + 4, y_top - 8, name, "t2", 11),
    ]


def bar(x0, y, w, h, cls):
    """A bar from the axis, its far end rounded."""
    r = min(3.0, w / 2, h / 2)
    if w <= 0:
        return ""
    return (
        f'<path class="{cls}" d="M{f(x0)} {f(y)}h{f(w - r)}a{f(r)} {f(r)} 0 0 1 {f(r)} {f(r)}'
        f'v{f(h - 2 * r)}a{f(r)} {f(r)} 0 0 1 {f(-r)} {f(r)}h{f(r - w)}z"/>'
    )


def hbars(title, sub, desc, unit, short, groups, series, refline=None):
    """Horizontal bars. groups: [(label, [value text or None per series])]; series: [(name, class)]."""
    lw = 12 + 7 * max(len(g) for g, _ in groups)
    x0, x1 = PAD + lw, W - PAD - 80
    top = 60 + 17 * len(sub) + (22 if len(series) > 1 else 0) + (14 if refline else 0)
    values = [float(v) for _, vs in groups for v in vs if v] + ([refline[0]] if refline else [])
    end, step = nice(max(values))
    bh, gap = 14, 2
    body = legend(PAD, top - 12 - (14 if refline else 0), series) if len(series) > 1 else []
    y = top
    for name, vs in groups:
        height = len(series) * bh + (len(series) - 1) * gap
        body.append(text(x0 - 10, y + height / 2 + 4, name, "t1", 12, "end"))
        for v, (_, cls) in zip(vs, series):
            if v:
                w = (x1 - x0) * float(v) / end
                body.append(bar(x0, y, w, bh, cls))
                body.append(text(x0 + w + 6, y + bh - 3, f"{v} {short}", "t1", 11))
            y += bh + gap
        y += 14 - gap
    bottom = y - 4
    if refline:
        body = ref(x0 + (x1 - x0) * refline[0] / end, top - 6, bottom, refline[1]) + body
    body = xaxis(x0, x1, top - 6, bottom, end, step, unit) + body
    return frame(title, sub, desc, int(bottom + 50), body)


def throughput():
    data = rows("throughput")
    steps = list(dict.fromkeys(r["step"] for r in data))
    lw = 12 + 7 * max(len(s) for s in steps)
    x0, x1 = PAD + lw, W - PAD - 20
    end, step = nice(max(float(r["mbps"]) for r in data))
    sub = [
        "Each dot is one run of cargo bench's speed bench: cat of 100 MB, MB/s of program output.",
        "Apple M6, release build. The runs far below the rest were on a busy machine (load 7 to 18).",
    ]
    top = 60 + 17 * len(sub) + 22 + 14
    body = legend(PAD, top - 26, [("before a change", "c0"), ("with it", "c1")])
    y = top + 8
    for s in steps:
        body.append(text(x0 - 10, y + 4, s, "t1", 12, "end"))
        for r in data:
            if r["step"] == s:
                x = x0 + (x1 - x0) * float(r["mbps"]) / end
                cls = "c1" if r["kind"] == "after" else "c0"
                body.append(f'<circle cx="{f(x)}" cy="{f(y)}" r="5" class="{cls} ring" stroke-width="2"/>')
        y += 22
    bottom = y - 8
    limit = ref(x0 + (x1 - x0) * 115 / end, top - 6, bottom, "the macOS PTY's own limit, about 115", True)
    body = xaxis(x0, x1, top - 6, bottom, end, step, "MB/s") + limit + body
    desc = (
        "cat throughput at each step from the reader thread to scrollback B: with each change leon's runs sit "
        "near the macOS PTY's own limit of about 115 MB/s; the runs far below it were on a loaded machine."
    )
    return frame("cat of 100 MB, run by run, at each step", sub, desc, int(bottom + 50), body)


def keystroke():
    data = rows("keystroke")
    groups = [(r["measurement"], [r["p50_ms"], r["p99_ms"]]) for r in data]
    sub = [
        "From the key's write until the app's client holds the screen that shows it (the speed bench).",
        "The budget is p50 under 4 ms and p99 under 16 ms: every run is far inside it.",
    ]
    desc = "Keystroke to screen model at each milestone: under 0.35 ms at p50 and under 0.8 ms at p99, through a link too."
    return hbars("Keystroke to screen, p50 and p99", sub, desc, "milliseconds", "ms", groups, [("p50", "c1"), ("p99", "c2")])


def key_to_pixel():
    data = rows("key-to-pixel")
    groups = [(r["measurement"], [r["p50_ms"], r["p99_ms"]]) for r in data]
    sub = [
        "In the app: key to the first paint that shows it. Outside: key to the frame composited, as a",
        "screen capture sees it (that tool's own floor is 18.8 ms at p50). 60 Hz display, load 2 to 4.5.",
    ]
    desc = (
        "Key to pixel: the app paints a key's echo 12 to 15 ms after the key at p50, idle or busy, mostly waiting "
        "for the next display tick; a screen capture sees it about 50 ms after the key."
    )
    return hbars("Key to pixel, p50 and p99", sub, desc, "milliseconds", "ms", groups, [("p50", "c1"), ("p99", "c2")], (16.7, "one 60 Hz frame"))


def memory_soak():
    data = rows("memory-soak")
    builds = list(dict.fromkeys(r["build"] for r in data))
    cls = {builds[0]: "c0", builds[1]: "c1"}
    minutes = max(int(r["minute"]) for r in data)
    end, step = nice(max(float(r["server_mib"]) for r in data))
    sub = [
        "soak.sh: 12 agent panes printing, and a session of 2 made and killed every minute.",
        "The server's footprint by minute. Apple M6, release build, 2026-11-19.",
    ]
    top = 60 + 17 * len(sub) + 46
    x0, x1, y0, y1 = PAD + 50, W - PAD - 150, top, top + 240
    body = legend(PAD, top - 34, [(b, cls[b]) for b in builds])
    for v in ticks(end, step):
        y = y1 - (y1 - y0) * v / end
        body.append(f'<line x1="{f(x0)}" y1="{f(y)}" x2="{f(x1)}" y2="{f(y)}" class="{"axis" if v == 0 else "grid"}" stroke-width="1"/>')
        body.append(text(x0 - 8, y + 4, label(v), "t2", 11, "end"))
    for m in range(minutes + 1):
        body.append(text(x0 + (x1 - x0) * m / minutes, y1 + 16, str(m), "t2", 11, "middle"))
    body.append(text((x0 + x1) / 2, y1 + 34, "minutes into the soak", "t2", 12, "middle"))
    body.append(text(x0 - 8, y0 - 14, "MiB", "t2", 12, "end"))
    for b in builds:
        pts = [(x0 + (x1 - x0) * int(r["minute"]) / minutes, y1 - (y1 - y0) * float(r["server_mib"]) / end, r) for r in data if r["build"] == b]
        body.append(f'<polyline fill="none" class="{cls[b]}" stroke-width="2" style="fill:none" points="{" ".join(f"{f(x)},{f(y)}" for x, y, _ in pts)}"/>')
        for x, y, _ in pts:
            body.append(f'<circle cx="{f(x)}" cy="{f(y)}" r="4" class="{cls[b]} ring" stroke-width="2"/>')
        x, y, last = pts[-1]
        body.append(text(x + 10, y + 4, f"{last['server_mib']} MiB at minute {last['minute']}", "t1", 12))
    desc = (
        "Server memory in the agent soak: before scrollback B it climbed from 359 to 562 MiB in 5 minutes; "
        "with leon's history store it fell to 36 MiB and stayed there (478 against 48 MiB over minutes 1 to 5)."
    )
    return frame("Server memory in an agent soak, before and with scrollback B", sub, desc, int(y1 + 50), body)


def memory_per_pane():
    data = rows("memory-per-pane")
    groups = [(r["build"], [r["mib_per_agent_pane"]]) for r in data]
    sub = ["Each agent pane's share of the soak's server memory, over the 4.9 MiB of an idle server.", "Apple M6, release build, 2026-11-19."]
    desc = "Memory per agent pane: about 33 MiB before scrollback B and 3 MiB with leon's history store, 90% less."
    return hbars("Memory per agent pane", sub, desc, "MiB per pane", "MiB", groups, [("MiB", "c1")])


def idle_cpu():
    data = rows("idle-cpu")
    groups = [(r["state"], [r["before_pct"], r["after_pct"]]) for r in data]
    sub = ["Leon.app with 12 idle panes (idle-cpu.sh). The server stays under 0.1% of a core in each.", "60 Hz display, release build."]
    desc = (
        "The app's CPU with 12 idle panes: 0.74% of a core when still and 0.06% when hidden, down from 2.3% and 1.9%, "
        "since it stopped polling every 8 ms and sleeps until woken."
    )
    return hbars("The app's CPU at rest", sub, desc, "% of one core", "%", groups, [("polling every 8 ms", "c0"), ("woken only by a change", "c1")])


def frame_build():
    data = rows("frame-build")
    groups = [(f"{r['panes']} pane{'s' if r['panes'] != '1' else ''} ({r['script']})", [r["p50_ms"]]) for r in data]
    sub = ["How long the app takes to build a frame at p50 while every pane prints.", "60 Hz display, release build."]
    desc = "Frame build at p50: 1.0 ms with one pane, 1.4 ms with 8 and 5.1 ms with 100 printing, all inside one 60 Hz frame of 16.7 ms."
    return hbars("Frame build time, p50", sub, desc, "milliseconds", "ms", groups, [("p50", "c1")], (16.7, "one 60 Hz frame"))


def flood():
    data = rows("flood")
    runs = list(dict.fromkeys(r["run"] for r in data))
    builds = list(dict.fromkeys(r["build"] for r in data))
    groups = [(f"run {n}", [next(r["mbps"] for r in data if r["run"] == n and r["build"] == b) for b in builds]) for n in runs]
    sub = ["flood.sh: cat of 100 MB of text and colour into one pane, with the app showing it.", "Apple M6, release build, alternating runs."]
    desc = "flood.sh's cat of 100 MB of text and colour: about 80 MB/s before the flood fix and about 115 MB/s with it, 44% faster."
    return hbars("A big cat of text and colour, before and with the fix", sub, desc, "MB/s", "MB/s", groups, [(builds[0], "c0"), (builds[1], "c1")])


CHARTS = {
    "throughput": throughput,
    "keystroke": keystroke,
    "key-to-pixel": key_to_pixel,
    "memory-soak": memory_soak,
    "memory-per-pane": memory_per_pane,
    "idle-cpu": idle_cpu,
    "frame-build": frame_build,
    "flood": flood,
}


def main(argv):
    check = argv == ["--check"]
    if argv and not check:
        print("usage: scripts/perf-charts.py [--check]", file=sys.stderr)
        return 2
    stale = []
    for name, draw in CHARTS.items():
        path = OUT / f"{name}.svg"
        svg = draw()
        if check:
            if not path.exists() or path.read_text(encoding="utf-8") != svg:
                stale.append(path.relative_to(ROOT).as_posix())
        else:
            OUT.mkdir(parents=True, exist_ok=True)
            path.write_text(svg, encoding="utf-8")
    extra = sorted(p.relative_to(ROOT).as_posix() for p in OUT.glob("*.svg") if p.stem not in CHARTS) if OUT.exists() else []
    unused = sorted(p.name for p in DATA.glob("*.csv") if p.stem not in CHARTS)
    for p in stale:
        print(f"perf-charts: {p} is not what its data draws: run python3 scripts/perf-charts.py")
    for p in extra:
        print(f"perf-charts: {p} has no data in docs/perf/")
    for p in unused:
        print(f"perf-charts: docs/perf/{p} draws no chart")
    return 1 if stale or extra or unused else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
