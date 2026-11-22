"""web.py: the web demo's cuts (DEMO_SCENES=web, scripts/demo-video/scenes-web.sh), for compose.py:
web is the whole take from its first mark; web-short (16:9), web-square (1:1) and web-vertical
(9:16) are the WEB_CUT pieces, each found by its mark. The square and vertical cuts frame each
piece by WEB_FRAMES: one region, or the two windows stacked, so the text stays readable on a phone."""
import sys

from PIL import Image

from draw import rounded_mask
from frame import MAIN, SQUARE, Layout

# the web cuts show the VM's own screen, edge to edge (no brand frame): the caption is a small
# opaque pill near the bottom, the keycaps above it. web-4k is the full cut at 3840x2160
WEB16 = Layout(1920, 1080, 0, 0, 1920, 1080, 0, 1010, 900)
WEB4K = Layout(3840, 2160, 0, 0, 3840, 2160, 0, 2020, 1800)
WEB_SQUARE = Layout(1080, 1080, 0, 0, 1080, 1080, 0, 1010, 900)
VERTICAL = Layout(1080, 1920, 0, 0, 1080, 1920, 0, 1830, 1700)
LAYOUTS = {"square": SQUARE, "web": WEB16, "web-4k": WEB4K, "web-short": WEB16, "web-square": WEB_SQUARE,
           "web-vertical": VERTICAL}
# the end card's lines: the headline, then what it is and where
WEB_END_LINES = ["Open it anywhere.", "leon 0.2.1 · your terminals in a browser on your tailnet",
                 "github.com/aindong/leon-cli"]


# the web demo's short cuts: (its scene's mark, seconds after it, seconds, caption, small line), so
# they follow the take's own beats (scripts/demo-video/scenes-web.sh). The first is the hook: both
# machines on one frame, the agent waiting in each
WEB_CUT = [
    ("4 answer", 0.6, 1.8, "Open it anywhere.", ""),
    ("2 settings", 4.4, 2.6, "Copy the address from Settings.", ""),
    ("3 browser", 2.8, 4.4, "Paste it into any browser.", "on your tailnet"),
    ("4 answer", 1.6, 3.0, "Answer your agents from it.", ""),
    ("5 phone", 3.4, 4.6, "Or from your phone.", ""),
]
# the square and vertical cuts' framing of each WEB_CUT piece, in the 3840x2160 take's pixels
# (kit/web.sh: the app at 24,40 pt and the browser at 968,40 pt, each about 920 x 950 pt). A list
# is a region the camera holds; a tuple, regions shown whole one under another, filling the frame:
# the app over the browser, their tops cut to the frame's shape
SETTINGS = [1490, 1050, 1240, 620]  # Settings' Web page part: the switch, the address, Copy, the QR
PHONE = [2440, 100, 880, 1800]
WEB_FRAMES = {
    "web-square": [((48, 80, 1840, 920), (1936, 80, 1856, 928)), SETTINGS, [1936, 80, 1856, 1856],
                   ((48, 80, 1840, 920), (1936, 80, 1856, 928)), PHONE],
    "web-vertical": [((48, 80, 1840, 1636), (1936, 80, 1856, 1650)), SETTINGS, [1936, 80, 1160, 1900],
                     ((48, 80, 1840, 1636), (1936, 80, 1856, 1650)), PHONE],
    # 16:9 follows the take's zooms, but holds Settings' Web page part: the "Serving on" line above
    # it says the seam's 127.0.0.1
    "web-short": [None, SETTINGS, None, None, None],
}
WEB_END = 2.5
WEB_HEAD = 0.3  # the web cut starts this long before the take's first mark


def web_pieces(mode, duration, cue, end):
    marks = {r["name"]: r["vt"] for r in cue if r["type"] == "mark"}
    if mode in ("web", "web-4k"):
        first = min(marks.values(), default=WEB_HEAD)
        return [("take", max(0.0, first - WEB_HEAD), duration, None, None), ("card", "end", end, WEB_END_LINES)]
    missing = [m for m, *_ in WEB_CUT if m not in marks]
    if missing:
        sys.exit(f"compose: the short cut needs the take's marks; the cue has no mark {', '.join(missing)}")
    frames = WEB_FRAMES.get(mode, [None] * len(WEB_CUT))
    out = [("take", marks[m] + a, min(duration, marks[m] + a + d), f, (c, sm))
           for (m, a, d, c, sm), f in zip(WEB_CUT, frames)]
    return out + [("card", "end", WEB_END, WEB_END_LINES)]


def layout(mode):
    return LAYOUTS.get(mode, MAIN)


def stacked(R, src, t, pt, regions, caption):
    """one frame of the regions, one under another, each the viewport's width (all scaled down
    together when they don't fit) and centred: the caption above, the keycaps below (no pointer:
    it would need a place in each region)"""
    L = R.L
    gap = 16 if L.vx else 0  # edge to edge: no gap, nothing behind
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
