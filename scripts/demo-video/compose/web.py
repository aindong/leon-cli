"""web.py: the web demo's cuts (DEMO_SCENES=web, scripts/demo-video/scenes-web.sh), for compose.py:
web is the whole take from its first mark; web-short (16:9), web-square (1:1) and web-vertical
(9:16) are the WEB_CUT pieces, each found by its mark. The square and vertical cuts frame each
piece by WEB_FRAMES: one region, or the two windows stacked, so the text stays readable on a phone."""
import sys

from PIL import Image

from draw import rounded_mask
from frame import MAIN, SQUARE, Layout

# 9:16 for Reels and Shorts: the take in the middle, the caption above it, the keycaps below
VERTICAL = Layout(1080, 1920, 24, 200, 1032, 1560, 22, 110, 1840)
LAYOUTS = {"square": SQUARE, "web-square": SQUARE, "web-vertical": VERTICAL}


# the web demo's short cut: (its scene's mark, seconds after it, seconds, caption, small line), so
# it follows the take's own beats (scripts/demo-video/scenes-web.sh)
WEB_CUT = [
    ("5 blocked", 1.6, 1.6, "Your Mac, and leon in a browser.", ""),  # a first frame that shows both
    ("2 settings", 2.8, 3.0, "Turn on the web page.", "Settings · Server"),
    ("3 laptop", 0.6, 3.2, "Open it on your other machine.", ""),
    ("4 typing", 0.6, 3.6, "Type from anywhere on your tailnet.", ""),
    ("5 blocked", 4.4, 3.4, "Answer your agents from the browser.", ""),
    ("6 phone", 1.0, 2.0, "Or from your phone.", ""),
]
# the square and vertical cuts' framing of each WEB_CUT piece, in raw pixels: [x, y, w, h], one
# region the camera holds; or ((x, y, w), (x, y, w)), two windows' tops stacked, the app over the
# page (kit/web.sh puts the app at 8,38 pt, 720 pt wide, and the page at 736,38 pt, 688 pt wide)
STACK = ((16, 76, 1440), (1472, 76, 1376))
WEB_FRAMES = [STACK, [1000, 740, 1300, 880], STACK, STACK, STACK, [2000, 76, 800, 1512]]
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
    frames = WEB_FRAMES if mode in ("web-square", "web-vertical") else [None] * len(WEB_CUT)
    out = [("take", marks[m] + a, min(duration, marks[m] + a + d), f, (c, sm))
           for (m, a, d, c, sm), f in zip(WEB_CUT, frames)]
    return out + [("card", "end", WEB_END)]


def layout(mode):
    return LAYOUTS.get(mode, MAIN)


def stacked(R, src, t, pt, regions, caption):
    """one frame of two windows' tops, one over the other, each the viewport's width: the caption
    above, the keycaps below (no pointer: it would need a place in each half)"""
    L, gap = R.L, 16
    sh = (L.vh - gap) // 2
    img = R.bg.copy()
    mask = rounded_mask(L.vw, sh, L.radius)
    for i, (x, y, w) in enumerate(regions):
        box = (x, y, x + w, y + w * sh / L.vw)
        img.paste(src.resize((L.vw, sh), Image.Resampling.LANCZOS, box=box), (L.vx, L.vy + i * (sh + gap)), mask)
    img = img.convert("RGBA")
    R.draw_keys(img, t)
    R.draw_caption(img, pt, [caption])
    return img.convert("RGB")


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
