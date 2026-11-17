"""The soak's chart and leak verdict: footprint of the server and the app over time.

python3 chart.py SOAK_CSV OUT_SVG
python3 chart.py selftest

A leak is growth that does not level off: the least-squares slope of the footprint over the last
60 minutes (samples right after a `leaks` pause left out), above 1 MiB/h for the server or 5 MiB/h
for the app (its glyph and GPU caches move more).
"""
import csv
import sys

LIMITS = {"server": 1.0, "app": 5.0}  # MiB per hour


def slope(points):
    n = len(points)
    if n < 2:
        return 0.0
    mx = sum(x for x, _ in points) / n
    my = sum(y for _, y in points) / n
    den = sum((x - mx) ** 2 for x, _ in points)
    return sum((x - mx) * (y - my) for x, y in points) / den if den else 0.0


def verdict(rows):
    out = []
    last = rows[-1]["minute"]
    for who in ("server", "app"):
        # a 0 is a sample `footprint` could not read: no verdict from it
        pts = [(r["minute"] / 60, r[f"{who}_footprint_kib"] / 1024) for r in rows
               if r["minute"] >= last - 60 and not r["after_leaks"] and r[f"{who}_footprint_kib"] > 0]
        if len(pts) < 10:
            out.append(f"{who}: only {len(pts)} measured samples in the last 60 minutes: no verdict")
            continue
        per_h = slope(pts)
        first, end = rows[0][f"{who}_footprint_kib"] / 1024, rows[-1][f"{who}_footprint_kib"] / 1024
        peak = max(r[f"{who}_footprint_kib"] for r in rows) / 1024
        leak = "LEAK" if per_h > LIMITS[who] else "levels off"
        out.append(f"{who}: footprint {first:.1f} MiB at minute 0, peak {peak:.1f}, {end:.1f} at minute {last}; "
                   f"last 60 min slope {per_h:+.2f} MiB/h (limit {LIMITS[who]}): {leak}")
    return out


def svg(rows, path):
    w, h, pad = 900, 420, 60
    xmax = max(1, rows[-1]["minute"])
    ymax = max(max(r["server_footprint_kib"], r["app_footprint_kib"]) for r in rows) / 1024 * 1.1 or 1
    x = lambda m: pad + (w - 2 * pad) * m / xmax
    y = lambda mib: h - pad - (h - 2 * pad) * mib / ymax
    parts = [f'<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" font-family="sans-serif" font-size="12">',
             f'<rect width="{w}" height="{h}" fill="white"/>',
             f'<text x="{pad}" y="24" font-size="15">leon soak: footprint over time</text>',
             f'<line x1="{pad}" y1="{h - pad}" x2="{w - pad}" y2="{h - pad}" stroke="#888"/>',
             f'<line x1="{pad}" y1="{pad}" x2="{pad}" y2="{h - pad}" stroke="#888"/>',
             f'<text x="{w / 2}" y="{h - 20}" text-anchor="middle">minutes</text>',
             f'<text x="16" y="{h / 2}" transform="rotate(-90 16 {h / 2})" text-anchor="middle">MiB</text>']
    for i in range(6):
        v = ymax * i / 5
        parts.append(f'<text x="{pad - 6}" y="{y(v) + 4:.0f}" text-anchor="end">{v:.0f}</text>')
        m = xmax * i / 5
        parts.append(f'<text x="{x(m):.0f}" y="{h - pad + 16}" text-anchor="middle">{m:.0f}</text>')
    for who, colour in (("server", "#1f6feb"), ("app", "#d1242f")):
        pts = " ".join(f"{x(r['minute']):.1f},{y(r[f'{who}_footprint_kib'] / 1024):.1f}" for r in rows)
        parts.append(f'<polyline fill="none" stroke="{colour}" stroke-width="2" points="{pts}"/>')
        lx = w - pad - (150 if who == "server" else 70)
        parts.append(f'<text x="{lx}" y="24" fill="{colour}">— {who}</text>')
    parts.append("</svg>")
    with open(path, "w") as f:
        f.write("\n".join(parts))


def read(path):
    with open(path) as f:
        return [{k: (float(v) if k == "load" else int(v)) for k, v in r.items()} for r in csv.DictReader(f)]


def selftest():
    flat = [{"minute": m, "server_footprint_kib": 20000 + (5000 if m < 10 else 6000), "app_footprint_kib": 90000,
             "after_leaks": 0} for m in range(121)]
    assert all("levels off" in l for l in verdict(flat)), verdict(flat)
    leaky = [dict(r, server_footprint_kib=20000 + r["minute"] * 40) for r in flat]  # 2.3 MiB/h
    out = verdict(leaky)
    assert "LEAK" in out[0] and "levels off" in out[1], out
    unread = [dict(r, app_footprint_kib=0) for r in flat]
    assert "no verdict" in verdict(unread)[1], verdict(unread)
    print("chart selftest ok")


if __name__ == "__main__":
    if sys.argv[1] == "selftest":
        selftest()
    else:
        rows = read(sys.argv[1])
        svg(rows, sys.argv[2])
        print("\n".join(verdict(rows)))
        print(f"chart: {sys.argv[2]}")
