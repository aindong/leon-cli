"""web.py: the web demo's two cuts (DEMO_SCENES=web, scripts/demo-video/scenes-web.sh), for compose.py:
web is the whole take from its first mark, web-short the WEB_CUT pieces, each found by its mark."""
import sys


# the web demo's short cut: (its scene's mark, seconds after it, seconds, caption, small line), so
# it follows the take's own beats (scripts/demo-video/scenes-web.sh)
WEB_CUT = [
    ("2 settings", 2.6, 3.4, "Turn on the web page.", "Settings · Tailscale"),
    ("3 laptop", 0.6, 3.2, "Open it on your other machine.", ""),
    ("4 typing", 0.6, 4.0, "Type from anywhere on your tailnet.", ""),
    ("5 blocked", 4.4, 3.4, "Answer your agents from the browser.", ""),
    ("6 phone", 1.0, 2.0, "Or from your phone.", ""),
]
WEB_END = 2.5
WEB_HEAD = 0.3  # the web cut starts this long before the take's first mark


def web_pieces(mode, duration, cue, end):
    marks = {r["name"]: r["vt"] for r in cue if r["type"] == "mark"}
    if mode == "web":
        first = min(marks.values(), default=WEB_HEAD)
        return [("take", max(0.0, first - WEB_HEAD), duration, None, None), ("card", "end", end)]
    missing = [m for m, *_ in WEB_CUT if m not in marks]
    if missing:
        sys.exit(f"compose: the short cut needs the take's marks; the cue has no mark {', '.join(missing)}")
    out = [("take", marks[m] + a, min(duration, marks[m] + a + d), None, (c, sm)) for m, a, d, c, sm in WEB_CUT]
    return out + [("card", "end", WEB_END)]


def web_srt_rows(ps, cue):
    """(start, end, text, small) on the output's clock: the short cut's own captions, or the take's"""
    rows, t = [], 0.0
    for p in ps:
        if p[0] == "card":
            continue
        if p[4] is not None:  # the short cut: each piece's own caption
            rows.append((t, t + p[2] - p[1], *p[4]))
        else:  # the whole take: its captions, from its start
            rows += [(t + max(c["vt"], p[1]) - p[1], t + min(c["vuntil"], p[2]) - p[1], c["text"], c.get("small", ""))
                     for c in cue if c["type"] == "caption" and c["vuntil"] > p[1]]
        t += p[2] - p[1]
    return rows
