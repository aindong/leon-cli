"""frame.py: one output frame: the layouts (16:9 and 1:1), the camera that eases onto each zoom
beat, and the renderer that puts the take, the pointer, the keycaps and the captions on the frame.
"""
import json
import math
from dataclasses import dataclass

import numpy as np
from PIL import Image, ImageDraw

from draw import (AMBER, CHALK, IRON, PARCHMENT, SYMBOLS, UMBER, background, badge, ease, font, pointer,
                  rounded_mask, shadow, symbol)

FPS = 60
SRC_W, SRC_H = 2880, 1620
# the part of the raw frame that is used: the right 32 px hold macOS's recording dot and the
# parked pointer; 9 px off the top and bottom keep it 16:9
BASE = (0.0, 9.0, 2848.0, 1602.0)
MOVE = 0.7  # seconds for a zoom in or out

# ---------------------------------------------------------------- layouts

@dataclass
class Layout:
    W: int
    H: int
    vx: int  # the take's viewport on the canvas
    vy: int
    vw: int
    vh: int
    radius: int
    caption_y: int  # the caption pill's centre line, over the take's top edge
    keys_y: int  # the keycaps' centre line, over its bottom edge


# the take fills most of the frame (a thin gradient border is left): on a phone its text is what
# has to read, so the caption and the keycaps sit on it as pills rather than in bands of their own
MAIN = Layout(1920, 1080, 64, 36, 1792, 1008, 22, 62, 1018)
SQUARE = Layout(1080, 1080, 24, 24, 1032, 1032, 22, 48, 1026)


def fit(region, aspect, base=BASE, max_zoom=1.8, margin=1.12):
    """the camera rect (cx, cy, w) that shows region with a margin, at the viewport's aspect"""
    x, y, w, h = region
    cx, cy = x + w / 2, y + h / 2
    bw, bh = base[2], base[3]
    cw = max(w * margin, h * margin * aspect, bw / max_zoom)
    cw = min(cw, bw, bh * aspect)
    ch = cw / aspect
    cx = min(max(cx, base[0] + cw / 2), base[0] + bw - cw / 2)
    cy = min(max(cy, base[1] + ch / 2), base[1] + bh - ch / 2)
    return cx, cy, cw


def lerp_cam(a, b, s):
    """between two camera rects: the width in log space, so a zoom feels even all the way"""
    w = math.exp(math.log(a[2]) + (math.log(b[2]) - math.log(a[2])) * s)
    return a[0] + (b[0] - a[0]) * s, a[1] + (b[1] - a[1]) * s, w


# ---------------------------------------------------------------- the cue file

def load_cue(path):
    rows = [json.loads(line) for line in open(path) if line.strip()]
    lead = next((r["lead"] for r in rows if r["type"] == "take"), 1.5)
    for r in rows:  # cue times are the script's clock; the video runs `lead` ahead
        r["vt"] = r["t"] + lead
        if "until" in r:
            r["vuntil"] = r["until"] + lead
    return rows


# ---------------------------------------------------------------- one output frame

class Renderer:
    def __init__(self, L, cue, blur=False):
        self.L, self.cue, self.blur = L, cue, blur
        self.aspect = L.vw / L.vh
        self.max_zoom = BASE[2] / L.vw  # one source pixel per output pixel at most
        self.bg = background(L.W, L.H)
        self.mask = rounded_mask(L.vw, L.vh, L.radius)
        self.shadow, self.spad = shadow(L)
        b = self.bg.convert("RGBA")
        b.alpha_composite(self.shadow, (L.vx - self.spad, L.vy - self.spad))
        self.base = b.convert("RGB")
        self.cursor = pointer(int(46 * L.W / 1920) if L.W == 1920 else 40)
        # no voice-over: the captions tell the story, so they read on a phone (a still 390 px wide)
        self.cap_font = font(56 if L.W == 1920 else 40, 640)
        self.small_font = font(38 if L.W == 1920 else 30, 520)
        self.badges = {}
        self.home = (BASE[0] + BASE[2] / 2, BASE[1] + BASE[3] / 2, min(BASE[2], BASE[3] * self.aspect))
        self.zooms = sorted((r for r in cue if r["type"] == "zoom"), key=lambda r: r["vt"])
        self.moves = self.keyframes()
        self.keys = [r for r in cue if r["type"] == "key" and r.get("show")]
        self.clicks = [r for r in cue if r["type"] == "click"]
        self.captions = [r for r in cue if r["type"] == "caption"]

    def keyframes(self):
        """(time, target) pairs: each zoom beat starts a move onto its region and its end a move
        home, unless the next beat starts within MOVE of that end: then the camera goes straight
        from one region to the next"""
        keys = []
        for i, z in enumerate(self.zooms):
            keys.append((z["vt"], fit(z["region"], self.aspect, max_zoom=self.max_zoom)))
            nxt = self.zooms[i + 1]["vt"] if i + 1 < len(self.zooms) else math.inf
            if nxt > z["vuntil"] + MOVE:
                keys.append((z["vuntil"], self.home))
        return keys

    # the camera for video time t (main): each move eases from where the camera was when it began
    def camera(self, t, fixed=None):
        if fixed is not None:
            return fixed
        cam, start, t_start, target = self.home, self.home, -math.inf, self.home
        for tk, goal in self.moves:
            if tk > t:
                break
            start = lerp_cam(start, target, ease((tk - t_start) / MOVE))  # where the last move had got to
            t_start, target = tk, goal
        if t_start == -math.inf:
            return cam
        return lerp_cam(start, target, ease((t - t_start) / MOVE))

    def view(self, src, cam):
        cx, cy, w = cam
        w = min(w, BASE[2], BASE[3] * self.aspect)
        h = w / self.aspect
        # inside the used part of the frame, rounding included (a hair below 0 is an error in PIL)
        x0 = min(max(cx - w / 2, BASE[0]), BASE[0] + BASE[2] - w)
        y0 = min(max(cy - h / 2, BASE[1]), BASE[1] + BASE[3] - h)
        box = (max(0.0, x0), max(0.0, y0), min(SRC_W, x0 + w), min(SRC_H, y0 + h))
        return src.resize((self.L.vw, self.L.vh), Image.Resampling.LANCZOS, box=box)

    def to_out(self, x, y, cam):
        cx, cy, w = cam
        h = w / self.aspect
        return (self.L.vx + (x - (cx - w / 2)) / w * self.L.vw, self.L.vy + (y - (cy - h / 2)) / h * self.L.vh)

    def frame(self, src, t, out_t, fixed=None, captions=None, show_cursor=True, keys=True):
        L = self.L
        cam = self.camera(t, fixed)
        if self.blur and fixed is None:
            prev, nxt = self.camera(t - 0.5 / FPS), self.camera(t + 0.5 / FPS)
            speed = abs(math.log(nxt[2] / prev[2])) + math.hypot(nxt[0] - prev[0], nxt[1] - prev[1]) / cam[2]
            if speed > 0.004:  # only while the camera moves fast enough to show it
                views = [self.view(src, lerp_cam(prev, nxt, s)) for s in (0.0, 0.5, 1.0)]
                v = np.mean([np.asarray(x, np.float32) for x in views], axis=0)
                view = Image.fromarray(v.astype(np.uint8))
            else:
                view = self.view(src, cam)
        else:
            view = self.view(src, cam)
        img = self.base.copy()
        img.paste(view, (L.vx, L.vy), self.mask)
        img = img.convert("RGBA")
        if show_cursor:
            self.draw_cursor(img, t, cam)
        if keys:
            self.draw_keys(img, t)
        self.draw_caption(img, out_t if captions is not None else t, captions)
        return img.convert("RGB")

    def draw_cursor(self, img, t, cam):
        L = self.L
        for c in self.clicks:
            tc = c["vt"]
            if not (tc - 1.0 <= t <= tc + 0.9):
                continue
            end = self.to_out(c["x"], c["y"], cam)
            start = (L.vx + L.vw * 0.92, L.vy + L.vh * 0.62)  # from where the pointer rests
            s = ease((t - (tc - 0.8)) / 0.7)
            # a gentle arc: the control point lifts the path a little
            ctrl = ((start[0] + end[0]) / 2, min(start[1], end[1]) - L.vh * 0.12)
            x = (1 - s) ** 2 * start[0] + 2 * (1 - s) * s * ctrl[0] + s * s * end[0]
            y = (1 - s) ** 2 * start[1] + 2 * (1 - s) * s * ctrl[1] + s * s * end[1]
            alpha = min(1.0, (t - (tc - 1.0)) / 0.2) * (1.0 if t < tc + 0.6 else max(0.0, 1 - (t - tc - 0.6) / 0.3))
            if tc <= t <= tc + 0.5:  # the ripple
                k = (t - tc) / 0.5
                r = 10 + 34 * ease(k)
                ring = Image.new("RGBA", img.size, (0, 0, 0, 0))
                ImageDraw.Draw(ring).ellipse((end[0] - r, end[1] - r, end[0] + r, end[1] + r),
                                             outline=AMBER + (int(220 * (1 - k)),), width=4)
                img.alpha_composite(ring)
            cur = self.cursor.copy()
            if alpha < 1:
                a = cur.getchannel("A").point(lambda v: int(v * alpha))
                cur.putalpha(a)
            img.alpha_composite(cur, (int(x - 2), int(y - 2)))

    def draw_keys(self, img, t):
        L = self.L
        shown = [k for k in self.keys if k["vt"] <= t <= k["vt"] + 1.4]
        if not shown:
            return
        k = shown[-1]  # a newer press replaces the one before
        nxt = next((n for n in self.keys if n["vt"] > k["vt"]), None)
        age = t - k["vt"]
        alpha = min(1.0, age / 0.08)
        if age > 1.15:
            alpha = max(0.0, 1 - (age - 1.15) / 0.25)
        if nxt is not None and nxt["vt"] - t < 0.12:
            alpha = min(alpha, max(0.0, (nxt["vt"] - t) / 0.12))
        b = self.badges.get(k["combo"])
        if b is None:
            b = self.badges[k["combo"]] = badge(k["combo"], 58 if L.W == 1920 else 54)
        rise = int((1 - ease(min(1.0, age / 0.18))) * 10)
        b2 = b
        if alpha < 1:
            b2 = b.copy()
            b2.putalpha(b.getchannel("A").point(lambda v: int(v * alpha)))
        img.alpha_composite(b2, (L.W // 2 - b.width // 2, L.keys_y - b.height // 2 + rise))

    def caption_runs(self, text):
        """the caption's runs of text, ⌘ ⇧ ⌥ ⌃ as symbol names, and each one's width"""
        f = self.cap_font
        marks = {"⌘": "cmd", "⇧": "shift", "⌥": "alt", "⌃": "ctrl"}
        runs, cur = [], ""
        for ch in text:
            if ch in marks:
                if cur:
                    runs.append(cur)
                runs.append(marks[ch])
                cur = ""
            else:
                cur += ch
        if cur:
            runs.append(cur)
        sym = int(f.size * 0.9)
        return runs, [sym + 2 if r in SYMBOLS else f.getlength(r) for r in runs]

    def caption_line(self, layer, runs, widths, x, y, alpha):
        """one caption line from x; the symbols drawn as strokes in the run of text"""
        f = self.cap_font
        sym = int(f.size * 0.9)
        d = ImageDraw.Draw(layer)
        for r, w in zip(runs, widths):
            if r in SYMBOLS:
                for dy, col, a in ((2, (15, 8, 10), 110), (0, PARCHMENT, 255)):
                    im = symbol(r, sym, col)
                    im.putalpha(im.getchannel("A").point(lambda v: int(v * a / 255 * alpha)))
                    layer.alpha_composite(im, (int(x), int(y - sym / 2 + dy)))
            else:
                d.text((x, y + 2), r, font=f, fill=(15, 8, 10, int(110 * alpha)), anchor="lm")
                d.text((x, y), r, font=f, fill=PARCHMENT + (int(255 * alpha),), anchor="lm")
            x += w

    def draw_caption(self, img, t, captions=None):
        L = self.L
        if captions is None:
            caps = [(c["vt"], c["vuntil"], c["text"], c.get("small", "")) for c in self.captions]
        else:
            caps = captions
        for a, b, text, small in caps:
            if not (a <= t <= b):
                continue
            alpha = min(1.0, (t - a) / 0.2, (b - t) / 0.2)
            layer = Image.new("RGBA", img.size, (0, 0, 0, 0))
            d = ImageDraw.Draw(layer)
            runs, widths = self.caption_runs(text)
            gap = 26  # between the caption and its small line, which follows it in the pill
            sw = self.small_font.getlength(small) + gap if small else 0
            w = sum(widths) + sw
            pad, h, y = 30, int(self.cap_font.size * 1.7), L.caption_y
            x = L.W / 2 - w / 2
            d.rounded_rectangle((x - pad, y - h / 2, x + w + pad, y + h / 2), h // 2, fill=IRON + (int(232 * alpha),),
                                outline=UMBER + (int(255 * alpha),), width=2)
            self.caption_line(layer, runs, widths, x, y, alpha)
            if small:
                d.text((x + sum(widths) + gap, y + 2), small, font=self.small_font, fill=CHALK + (int(230 * alpha),),
                       anchor="lm")
            img.alpha_composite(layer)
