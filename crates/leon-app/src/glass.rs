//! The translucent window, the GPUI-free part: how opaque the chrome and each pane are drawn for
//! a `translucency` setting, and where the panes area's gaps are. GPUI's renderer blends colour
//! over what is drawn but adds alpha (`metal_renderer.rs`, destination alpha factor `One`), so
//! every pixel gets at most one translucent fill: the gaps are painted around the tiles, never
//! under them.

use leon_lua::settings::TRANSLUCENCY_MAX;

use crate::panes::Rect;
use crate::theme::{contrast, mix, Theme};

/// Body text keeps this contrast over any desktop (WCAG AA), or its opaque contrast if lower.
pub const BODY: f64 = 4.5;
/// A tile's corner radius: `pane_tile` rounds by it, and the gaps go round it.
pub const TILE_RADIUS: f32 = 6.;
/// The desktops checked: white and black, the extremes any desktop lies between.
const DESKS: [u32; 2] = [u32::MAX >> 8, 0];
/// Secondary text on the chrome (session labels, shortcuts) keeps this much.
pub const SECONDARY: f64 = 3.0;
/// How much more of the desktop macOS shows than a fill's alpha says: measured on screen over
/// white and black (a pane at alpha 0.733 composited as about 0.69, the chrome at 0.776 as about
/// 0.75), with some margin. The readability floor is reckoned with it.
pub const SEE_GAIN: f32 = 1.25;

/// The alpha a fill drawn at `a` shows the desktop through with, as measured.
fn shown(a: f32) -> f32 {
    (1. - SEE_GAIN * (1. - a)).max(0.)
}

/// How opaque the window's fills are drawn: `see` is the setting, `chrome` the chrome's alpha and
/// `pane` every pane's.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Glass {
    see: f32,
    chrome: f32,
    pane: f32,
}

impl Glass {
    /// Today's window: every fill opaque.
    pub const OPAQUE: Glass = Glass { see: 0., chrome: 1., pane: 1. };

    /// The fills for `window`'s theme with `see` of the desktop showing through (0 to
    /// `TRANSLUCENCY_MAX`; outside it is clamped, NaN is opaque), panes without the focus faded
    /// by `fade`. Every pane gets one alpha, enough for the theme faded or not: a faded pane
    /// keeps the contrast its fade gives it, and the focused pane is never the paler one.
    pub fn new(window: &Theme, see: f32, fade: f32) -> Glass {
        let see = if see.is_nan() { 0. } else { see.clamp(0., TRANSLUCENCY_MAX as f32) };
        if see == 0. {
            return Glass::OPAQUE;
        }
        let chrome =
            alpha_for(&[(window.chrome, window.text, BODY), (window.chrome, window.muted, SECONDARY)], 1. - see);
        let f = crate::fade::theme(window, fade);
        let pane = alpha_for(&[(window.bg, window.fg, BODY), (f.bg, f.fg, BODY)], 1. - see);
        Glass { see, chrome, pane }
    }

    /// Whether the desktop shows through: the window's background is blurred.
    pub fn translucent(&self) -> bool {
        self.see > 0.
    }

    /// The alpha of the sidebar, the title bar and the gaps between panes.
    pub fn chrome_alpha(&self) -> f32 {
        self.chrome
    }

    /// The alpha of every pane: what the setting asks, or more, until `fg` reads over a white
    /// and a black desktop as it must, faded or not.
    pub fn pane_alpha(&self) -> f32 {
        self.pane
    }
}

/// `want`, or the least alpha above it at which each (fill, text, contrast) keeps that contrast
/// (capped at what the text has on the fill opaque) over a white and over a black desktop.
fn alpha_for(texts: &[(u32, u32, f64)], want: f32) -> f32 {
    if want >= 1. {
        return 1.;
    }
    let bad: Vec<(f32, f32)> = texts
        .iter()
        .flat_map(|&(fill, text, need)| DESKS.map(|desk| too_low(text, need.min(contrast(text, fill)), fill, desk)))
        .flatten()
        .collect();
    let mut a = want;
    // each step leaves one interval for good, so this ends
    while let Some(&(_, hi)) = bad.iter().find(|&&(lo, hi)| a > lo && a < hi) {
        a = hi;
    }
    a
}

/// The alphas at which `text` on `fill` over `desk` is below `need`: an interval, since the fill
/// over the desk moves steadily from the desk's colour to its own, so the contrast falls until it
/// passes the text's lightness and then rises (to at least `need` when opaque). Found by
/// bisection, the same for any `want`, so more translucency never asks for a more opaque fill.
fn too_low(text: u32, need: f64, fill: u32, desk: u32) -> Option<(f32, f32)> {
    let c = |a: f32| contrast(text, mix(desk, fill, shown(a)));
    let ok = |a: f32| c(a) >= need - 1e-9;
    // the lowest contrast, by ternary search
    let (mut l, mut r) = (0f32, 1f32);
    for _ in 0..40 {
        let (m1, m2) = (l + (r - l) / 3., r - (r - l) / 3.);
        if c(m1) > c(m2) {
            l = m1;
        } else {
            r = m2;
        }
    }
    let low = (l + r) / 2.;
    if ok(low) {
        return None;
    }
    // the edges, each side of it: ok outside, not ok inside
    let edge = |mut out: f32, mut inside: f32| {
        for _ in 0..24 {
            let mid = (out + inside) / 2.;
            if ok(mid) {
                out = mid;
            } else {
                inside = mid;
            }
        }
        out
    };
    // below 0: bad from 0 itself
    let lo = if ok(0.) { edge(0., low) } else { -1. };
    Some((lo, edge(1., low)))
}

/// The lower contrast of `text` on `layers` (colour, alpha; bottom first) drawn over a white and
/// over a black desktop, blended per channel as the window server composites them (`shown`).
pub fn worst_contrast(text: u32, layers: &[(u32, f32)]) -> f64 {
    DESKS
        .map(|desk| contrast(text, layers.iter().fold(desk, |under, &(c, a)| mix(under, c, shown(a)))))
        .into_iter()
        .fold(f64::INFINITY, f64::min)
}

/// What the panes area fills with the panes' translucent background: the tiles and the drawers in
/// place that `shown` keeps. A drawer sliding out is drawn opaque, like a float: the tiles took its
/// room back as it started (`floats::place`), so it slides over them.
pub fn see_through_tiles(
    l: &crate::panes::Layout,
    placed: &crate::floats::Placed,
    shown: impl Fn(&str) -> bool,
) -> Vec<Rect> {
    let tiles = l.panes.iter().filter(|p| shown(&p.block_id)).map(|p| p.rect);
    tiles.chain(placed.docks.iter().filter(|d| !d.leaving && shown(&d.block_id)).map(|d| d.rect)).collect()
}

/// A piece of gap outside a tile's rounded corner: what of the ring around `center` from
/// `radius` outward falls in `clip` (the corner's square, in the area).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Corner {
    pub center: (f32, f32),
    pub radius: f32,
    pub clip: Rect,
}

/// What of a panes area no tile covers: rects, and the corners the tiles' rounding leaves.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Gaps {
    pub rects: Vec<Rect>,
    pub corners: Vec<Corner>,
}

/// The parts of `area` outside every tile (tiles and drawers, which never overlap), each tile's
/// corners rounded by `radius`: strips between the tiles' x edges, less the tiles that span them.
pub fn gaps(area: Rect, tiles: &[Rect], radius: f32) -> Gaps {
    let (x1, y1) = (area.x + area.w, area.y + area.h);
    let clip = |r: &Rect| {
        let (l, t) = (r.x.max(area.x), r.y.max(area.y));
        let (rr, b) = ((r.x + r.w).min(x1), (r.y + r.h).min(y1));
        (rr > l && b > t).then_some(Rect { x: l, y: t, w: rr - l, h: b - t })
    };
    let shown: Vec<Rect> = tiles.iter().filter_map(clip).collect();
    let mut xs: Vec<f32> = [area.x, x1].into_iter().chain(shown.iter().flat_map(|r| [r.x, r.x + r.w])).collect();
    xs.sort_by(f32::total_cmp);
    xs.dedup();
    let mut out = Gaps::default();
    for w in xs.windows(2) {
        let (l, r) = (w[0], w[1]);
        let mut spans: Vec<(f32, f32)> =
            shown.iter().filter(|t| t.x <= l && t.x + t.w >= r).map(|t| (t.y, t.y + t.h)).collect();
        spans.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut y = area.y;
        for (top, bottom) in spans.into_iter().chain([(y1, y1)]) {
            if top > y {
                out.rects.push(Rect { x: l, y, w: r - l, h: top - y });
            }
            y = y.max(bottom);
        }
    }
    if radius > 0. {
        for t in tiles {
            let r = radius.min(t.w / 2.).min(t.h / 2.);
            for (cx, cy, bx, by) in [
                (t.x + r, t.y + r, t.x, t.y),
                (t.x + t.w - r, t.y + r, t.x + t.w - r, t.y),
                (t.x + r, t.y + t.h - r, t.x, t.y + t.h - r),
                (t.x + t.w - r, t.y + t.h - r, t.x + t.w - r, t.y + t.h - r),
            ] {
                if let Some(c) = clip(&Rect { x: bx, y: by, w: r, h: r }) {
                    out.corners.push(Corner { center: (cx, cy), radius: r, clip: c });
                }
            }
        }
    }
    out
}
