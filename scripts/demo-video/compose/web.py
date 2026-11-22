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
    ("2 settings", 7.4, 2.8, "Turn on the web page.", "then Copy its address"),
    ("3 laptop", 2.6, 4.0, "Open it on your other machine.", "paste the address"),
    ("4 typing", 0.6, 3.6, "Type from anywhere on your tailnet.", ""),
    ("5 blocked", 4.4, 3.4, "Answer your agents from the browser.", ""),
    ("6 phone", 1.0, 1.6, "Or from your phone.", ""),
]
# the square and vertical cuts' framing of each WEB_CUT piece: regions of the raw frame (pixels,
# x y w h), each shown whole at the frame's width, one under the other, scaled down to fit. Two:
# the app over the page (kit/web.sh puts the app at 8,38 pt, 720 pt wide, the page at 736,38 pt,
# 688 pt wide), their tops; one: Settings' switches and address, or the phone-sized page
SETTINGS = ((1010, 720, 1250, 640),)  # scrolled: the Web page switch, the address, Copy, the QR
PHONE = ((2000, 76, 800, 1512),)
WEB_FRAMES = {
    "web-square": [((16, 76, 1440, 700), (1472, 76, 1376, 670))] * 6,
    "web-vertical": [((16, 76, 1440, 1080), (1472, 76, 1376, 1032))] * 6,
}
for frames in WEB_FRAMES.values():
    frames[1], frames[5] = SETTINGS, PHONE
# 16:9 follows the take's zooms, but Settings is the Web page part alone there too: a 16:9 camera
# can't leave out the "Serving on" line above it (the seam's 127.0.0.1)
WEB_FRAMES["web-short"] = [None, SETTINGS, None, None, None, None]
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
    frames = WEB_FRAMES.get(mode, [None] * len(WEB_CUT))
    out = [("take", marks[m] + a, min(duration, marks[m] + a + d), f, (c, sm))
           for (m, a, d, c, sm), f in zip(WEB_CUT, frames)]
    return out + [("card", "end", WEB_END)]


def layout(mode):
    return LAYOUTS.get(mode, MAIN)


def stacked(R, src, t, pt, regions, caption):
    """one frame of the regions, one under another, each the viewport's width (all scaled down
    together when they don't fit) and centred: the caption above, the keycaps below (no pointer:
    it would need a place in each region)"""
    L, gap = R.L, 16
    sizes = [(L.vw, L.vw * h / w) for _, _, w, h in regions]
    k = min(1.0, (L.vh - gap * (len(regions) - 1)) / sum(h for _, h in sizes))
    sizes = [(round(w * k), round(h * k)) for w, h in sizes]
    y = L.vy + (L.vh - sum(h for _, h in sizes) - gap * (len(regions) - 1)) // 2
    img = R.bg.copy()
    for (x0, y0, w, h), (sw, sh) in zip(regions, sizes):
        view = src.resize((sw, sh), Image.Resampling.LANCZOS, box=(x0, y0, x0 + w, y0 + h))
        img.paste(view, (L.vx + (L.vw - sw) // 2, y), rounded_mask(sw, sh, L.radius))
        y += sh + gap
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
