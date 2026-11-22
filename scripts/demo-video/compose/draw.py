"""draw.py: what the compositor draws that isn't the take: the brand gradient, the frame's mask
and shadow, keycaps and their symbols, the pointer, the wordmark and the title and end cards. All of
it is drawn in code, in an original style; nothing is copied from other artwork.
"""
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw, ImageFilter, ImageFont

REPO = Path(__file__).resolve().parent.parent.parent.parent
# Instrument Sans, the brand's font (SIL OFL 1.1, brand/fonts/OFL.txt), read as the repo has it
FONT = REPO / "brand" / "fonts" / "instrument-sans-latin.woff2"

# brand palette (brand/README.md)
DEEP_MAROON, MAROON, OXIDE = (0x35, 0x20, 0x28), (0x54, 0x2D, 0x35), (0xB6, 0x4C, 0x32)
AMBER, GOLD, CORAL = (0xF3, 0xA7, 0x47), (0xFF, 0xD4, 0x77), (0xEB, 0x79, 0x5D)
IRON, PARCHMENT, CHALK, UMBER = (0x29, 0x25, 0x21), (0xF5, 0xF0, 0xE7), (0xD8, 0xCE, 0xC2), (0x65, 0x59, 0x4F)


def font(size, weight=600):
    f = ImageFont.truetype(str(FONT), size)
    f.set_variation_by_axes([weight])
    return f

def ease(x):
    """smootherstep: no jump in speed or acceleration at either end"""
    x = min(1.0, max(0.0, x))
    return x * x * x * (x * (6 * x - 15) + 10)



def background(W, H):
    """a diagonal gradient through the brand's warm darks, with a soft amber glow top left"""
    yy, xx = np.mgrid[0:H, 0:W].astype(np.float32)
    t = (xx / W * 0.65 + yy / H * 0.35)
    stops = [(0.0, DEEP_MAROON), (0.55, MAROON), (1.0, OXIDE)]
    out = np.zeros((H, W, 3), np.float32)
    for (t0, c0), (t1, c1) in zip(stops, stops[1:]):
        m = (t >= t0) & (t <= t1)
        f = ((t - t0) / (t1 - t0))[m][:, None]
        out[m] = np.array(c0) * (1 - f) + np.array(c1) * f
    glow = np.exp(-(((xx - W * 0.12) / (W * 0.45)) ** 2 + ((yy - H * 0.05) / (H * 0.6)) ** 2))[..., None]
    out = out * (1 - 0.18 * glow) + np.array(AMBER, np.float32) * 0.18 * glow
    noise = np.random.default_rng(7).normal(0, 1.2, (H, W, 1))  # a little grain: no banding
    return Image.fromarray(np.clip(out + noise, 0, 255).astype(np.uint8))


def rounded_mask(w, h, r, scale=4):
    m = Image.new("L", (w * scale, h * scale), 0)
    ImageDraw.Draw(m).rounded_rectangle((0, 0, w * scale - 1, h * scale - 1), r * scale, fill=255)
    return m.resize((w, h), Image.Resampling.LANCZOS)


def shadow(L):
    pad = 80
    s = Image.new("L", (L.vw + 2 * pad, L.vh + 2 * pad), 0)
    ImageDraw.Draw(s).rounded_rectangle((pad, pad + 14, pad + L.vw, pad + L.vh + 14), L.radius, fill=150)
    s = s.filter(ImageFilter.GaussianBlur(28))
    rgba = Image.new("RGBA", s.size, (20, 10, 12, 0))
    rgba.putalpha(s)
    return rgba, pad


def symbol(name, size, color):
    """the macOS key symbols, drawn here as strokes (an original style, one weight for all)"""
    S = 4
    n = size * S
    img = Image.new("RGBA", (n, n), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    lw = max(2, int(n * 0.085))
    c = color + (255,)
    m = n * 0.18  # margin

    def line(pts):
        d.line(pts, fill=c, width=lw, joint="curve")
        for p in (pts[0], pts[-1]):
            d.ellipse((p[0] - lw / 2, p[1] - lw / 2, p[0] + lw / 2, p[1] + lw / 2), fill=c)

    if name == "cmd":  # a square whose sides run on into a loop at each corner
        a, b, r = n * 0.38, n * 0.62, n * 0.13
        for p0, p1 in (((a - r, a), (b + r, a)), ((a - r, b), (b + r, b)), ((a, a - r), (a, b + r)), ((b, a - r), (b, b + r))):
            line([p0, p1])
        # PIL's angles run clockwise from 3 o'clock; each loop goes the long way round
        for cx, cy, a0, a1 in ((a - r, a - r, 90, 360), (b + r, a - r, 180, 450), (b + r, b + r, 270, 540),
                               (a - r, b + r, 0, 270)):
            d.arc((cx - r - lw / 2, cy - r - lw / 2, cx + r + lw / 2, cy + r + lw / 2), a0, a1, fill=c, width=lw)
    elif name == "shift":  # an outlined up arrow
        pts = [(n / 2, m), (n - m, n * 0.5), (n * 0.66, n * 0.5), (n * 0.66, n - m), (n * 0.34, n - m),
               (n * 0.34, n * 0.5), (m, n * 0.5), (n / 2, m)]
        line(pts)
    elif name == "alt":  # the option key: a raised line, a slope, and a short bar
        line([(m, n * 0.32), (n * 0.38, n * 0.32), (n * 0.66, n * 0.7), (n - m, n * 0.7)])
        line([(n * 0.6, n * 0.32), (n - m, n * 0.32)])
    elif name == "ctrl":  # a caret
        line([(n * 0.24, n * 0.6), (n / 2, n * 0.32), (n * 0.76, n * 0.6)])
    elif name == "return":  # a hooked arrow
        line([(n - m, n * 0.24), (n - m, n * 0.58), (m + n * 0.06, n * 0.58)])
        line([(m + n * 0.24, n * 0.4), (m + n * 0.06, n * 0.58), (m + n * 0.24, n * 0.76)])
    elif name == "esc":  # a broken circle with an arrow out of it
        r = n * 0.3
        d.arc((n / 2 - r, n / 2 - r + n * 0.04, n / 2 + r, n / 2 + r + n * 0.04), 300, 240, fill=c, width=lw)
        line([(n * 0.42, n * 0.5), (n * 0.2, n * 0.28)])
        line([(n * 0.2, n * 0.44), (n * 0.2, n * 0.28), (n * 0.36, n * 0.28)])
    elif name in ("left", "right", "up", "down"):
        line([(m, n / 2), (n - m, n / 2)])
        line([(m + n * 0.2, n * 0.3), (m, n / 2), (m + n * 0.2, n * 0.7)])
        img = img.rotate({"left": 0, "up": -90, "right": 180, "down": 90}[name])
    return img.resize((size, size), Image.Resampling.LANCZOS)


SYMBOLS = {"cmd", "shift", "alt", "ctrl", "return", "esc", "left", "right", "up", "down"}


def keycap(label, h):
    """one key: a warm dark cap with a lighter top edge; a symbol, or the key's letter"""
    S = 3
    f = font(int(h * 0.46 * S), 620)
    if label in SYMBOLS:
        w = h
    else:
        text = label.upper() if len(label) == 1 else label
        tw = f.getlength(text) / S
        w = max(h, int(tw + h * 0.62))
    img = Image.new("RGBA", (w * S, h * S + 6 * S), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    r = int(h * 0.22 * S)
    d.rounded_rectangle((0, 4 * S, w * S - 1, h * S + 6 * S - 1), r, fill=(18, 12, 12, 170))  # the drop
    d.rounded_rectangle((0, 0, w * S - 1, h * S - 1), r, fill=IRON + (240,), outline=UMBER + (255,), width=2 * S)
    d.rounded_rectangle((3 * S, 3 * S, w * S - 3 * S, h * S * 0.5), r - 2 * S, fill=(255, 255, 255, 14))
    if label in SYMBOLS:
        sym = symbol(label, int(h * 0.56 * S), PARCHMENT)
        img.alpha_composite(sym, ((w * S - sym.width) // 2, (h * S - sym.height) // 2))
    else:
        d.text((w * S / 2, h * S / 2), text, font=f, fill=PARCHMENT, anchor="mm")
    return img.resize((w, h + 6), Image.Resampling.LANCZOS)


KEYNAMES = {"cmd": "cmd", "shift": "shift", "alt": "alt", "ctrl": "ctrl", "return": "return", "esc": "esc ",
            "left": "left", "right": "right", "up": "up", "down": "down", "space": "space", "tab": "tab"}


def combo_caps(combo):
    parts = combo.split("+")
    key = parts[-1] if parts[-1] else "+"
    mods = [p for p in parts[:-1] if p]
    order = ["ctrl", "alt", "shift", "cmd"]  # macOS's own order
    mods.sort(key=order.index)
    return mods + [KEYNAMES.get(key, key)]


def badge(combo, h=58, gap=10):
    caps = [keycap(c, h) for c in combo_caps(combo)]
    w = sum(c.width for c in caps) + gap * (len(caps) - 1)
    out = Image.new("RGBA", (w, caps[0].height), (0, 0, 0, 0))
    x = 0
    for c in caps:
        out.alpha_composite(c, (x, 0))
        x += c.width + gap
    return out


def pointer(size=44):
    """an original pointer: a rounded arrowhead with a short tail, light with a dark rim"""
    S = 4
    n = size * S
    img = Image.new("RGBA", (n, n), (0, 0, 0, 0))
    pts = [(0.06, 0.04), (0.06, 0.78), (0.26, 0.6), (0.4, 0.92), (0.54, 0.86), (0.4, 0.55), (0.68, 0.55)]
    poly = [(x * n * 0.95 + n * 0.04, y * n * 0.95 + n * 0.03) for x, y in pts]
    sh = Image.new("L", (n, n), 0)
    ImageDraw.Draw(sh).polygon([(x + 2 * S, y + 3 * S) for x, y in poly], fill=120)
    sh = sh.filter(ImageFilter.GaussianBlur(3 * S))
    img.putalpha(sh)
    img = Image.composite(Image.new("RGBA", (n, n), (10, 6, 6, 255)), Image.new("RGBA", (n, n), (0, 0, 0, 0)), sh)
    d = ImageDraw.Draw(img)
    d.polygon(poly, fill=IRON + (255,))
    inner = [(x * 0.86 + n * 0.035, y * 0.86 + n * 0.05) for x, y in poly]
    d.polygon(inner, fill=PARCHMENT + (255,))
    return img.resize((size, size), Image.Resampling.LANCZOS)


def wordmark(height, color=PARCHMENT):
    """brand/wordmark-dark.svg's strokes (M, V, H, Q, C, Z), flattened and drawn here"""
    import re
    svg = (REPO / "brand" / "wordmark-dark.svg").read_text()
    paths = re.findall(r' d="([^"]+)"', svg)
    S = 8
    scale = height / 90 * S
    img = Image.new("RGBA", (int(260 * scale), int(90 * scale)), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    lw = int(12 * scale)
    for p in paths:
        toks = re.findall(r"[A-Za-z]|-?[\d.]+", p)
        pts, cur, start, i = [], (0, 0), (0, 0), 0
        cmd = None
        while i < len(toks):
            if toks[i].isalpha():
                cmd = toks[i]
                i += 1
            nums = lambda k: [float(v) for v in toks[i:i + k]]
            if cmd == "M":
                cur = start = tuple(nums(2)); i += 2
                if pts:
                    pts.append(None)
                pts.append(cur)
            elif cmd == "V":
                cur = (cur[0], nums(1)[0]); i += 1; pts.append(cur)
            elif cmd == "H":
                cur = (nums(1)[0], cur[1]); i += 1; pts.append(cur)
            elif cmd == "L":
                cur = tuple(nums(2)); i += 2; pts.append(cur)
            elif cmd == "Q":
                c1x, c1y, x, y = nums(4); i += 4
                for k in range(1, 25):
                    t = k / 24
                    pts.append(((1 - t) ** 2 * cur[0] + 2 * (1 - t) * t * c1x + t * t * x,
                                (1 - t) ** 2 * cur[1] + 2 * (1 - t) * t * c1y + t * t * y))
                cur = (x, y)
            elif cmd == "C":
                c1x, c1y, c2x, c2y, x, y = nums(6); i += 6
                for k in range(1, 33):
                    t = k / 32
                    mt = 1 - t
                    pts.append((mt ** 3 * cur[0] + 3 * mt * mt * t * c1x + 3 * mt * t * t * c2x + t ** 3 * x,
                                mt ** 3 * cur[1] + 3 * mt * mt * t * c1y + 3 * mt * t * t * c2y + t ** 3 * y))
                cur = (x, y)
            elif cmd in ("Z", "z"):
                pts.append(start); cur = start
            else:
                i += 1
        run = []
        for q in pts + [None]:
            if q is None:
                if len(run) > 1:
                    sp = [(x * scale, y * scale) for x, y in run]
                    for p0, p1 in zip(sp, sp[1:]):
                        d.line([p0, p1], fill=color + (255,), width=lw)
                    for x, y in sp:
                        d.ellipse((x - lw / 2, y - lw / 2, x + lw / 2, y + lw / 2), fill=color + (255,))
                run = []
            else:
                run.append(q)
    return img.resize((int(260 * height / 90), height), Image.Resampling.LANCZOS)


def card(L, kind, t, dur, end_lines=None):
    """the title card and the end card: the lion, the wordmark, a line or three"""
    img = background(L.W, L.H).convert("RGBA") if not hasattr(card, "bg") or card.bg.size != (L.W, L.H) else card.bg.copy()
    card.bg = img.copy()
    icon = Image.open(REPO / "brand" / "icon" / "leon-1024.png").convert("RGBA")
    k = L.W / 1920 if L.W >= 1920 else 1.0  # 4K: everything twice 1080p's
    s = round(250 * k) if L.W >= 1920 else 230
    icon = icon.resize((s, s), Image.Resampling.LANCZOS)
    wm = wordmark(round(110 * k) if L.W >= 1920 else 96)
    a = ease(min(1.0, t / 0.5)) * (1 - ease(max(0.0, (t - (dur - 0.4)) / 0.4)))
    cy = L.H // 2 - round((60 if kind == "title" else 120) * k)
    rise = int((1 - ease(min(1.0, t / 0.6))) * 18 * k)
    layer = Image.new("RGBA", img.size, (0, 0, 0, 0))
    layer.alpha_composite(icon, (L.W // 2 - s // 2, cy - s // 2 - round(70 * k) + rise))
    layer.alpha_composite(wm, (L.W // 2 - wm.width // 2, cy + s // 2 - round(40 * k) + rise))
    d = ImageDraw.Draw(layer)
    y = cy + s // 2 + wm.height
    lines = ["Keep your work in view."] if kind == "title" else (end_lines or [])
    for i, line in enumerate(lines):
        f = font(round((40 if i == 0 else 32) * k), 560 if i == 0 else 480)
        d.text((L.W / 2, y + 10 * k + i * 56 * k), line, font=f, fill=(PARCHMENT if i == 0 else CHALK) + (255,), anchor="mm")
    layer.putalpha(layer.getchannel("A").point(lambda v: int(v * a)))
    img.alpha_composite(layer)
    return img.convert("RGB")
