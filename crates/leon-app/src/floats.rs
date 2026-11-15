//! Where a window's floats and drawers go in the pane area (M20b), which one a point hits, what a
//! drag does to them, and how far a drawer has slid. GPUI-free, so `leon attach` shares the rules.

use std::time::Duration;

use leon_core::overlays::{Bounds, DockSide, Overlays};
use leon_core::Window;

use crate::panes::{Rect, INSET};

/// A drawer: its tile, and the channel between it and the tiles that a drag resizes (none while
/// it slides).
#[derive(Debug, Clone, PartialEq)]
pub struct Dock {
    pub block_id: String,
    pub side: DockSide,
    /// The share of the area it has, which a full side can make less than its `size`.
    pub size: f64,
    pub rect: Rect,
    pub handle: Option<Rect>,
    /// Hidden, and drawn only while it slides out: clicks go to what is under it.
    pub leaving: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Float {
    pub block_id: String,
    pub bounds: Bounds,
    pub rect: Rect,
}

/// The pane area split up: the tiles' part, the drawers, and the floats back to front.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Placed {
    pub tiles: Rect,
    pub docks: Vec<Dock>,
    pub floats: Vec<Float>,
}

impl Placed {
    /// The overlay under (x, y): the topmost float, else a drawer; None over the tiles.
    pub fn hit(&self, x: f32, y: f32) -> Option<&str> {
        let inside = |r: &Rect| x >= r.x && x < r.x + r.w && y >= r.y && y < r.y + r.h;
        let float = self.floats.iter().rev().find(|f| inside(&f.rect)).map(|f| f.block_id.as_str());
        float.or_else(|| self.docks.iter().find(|d| inside(&d.rect)).map(|d| d.block_id.as_str()))
    }
}

/// Lay out `o` in a pane area of `size`. Visible drawers take their side's share in list order
/// (clamped to what is left); the tiles get the rest; floats sit at their bounds of the whole
/// area. `sliding` gives a drawer's shown fraction while it slides: a shown one keeps its room and
/// is drawn that far in, a hidden one (sliding out) takes no room.
pub fn place(o: &Overlays, size: (f32, f32), sliding: &dyn Fn(&str) -> Option<f32>) -> Placed {
    let mut tiles = Rect { x: 0., y: 0., w: size.0, h: size.1 };
    let mut docks = Vec::new();
    for d in &o.docked {
        let shown = sliding(&d.block_id);
        if d.hidden && shown.is_none() {
            continue;
        }
        let (region, handle) = match d.side {
            DockSide::Left => {
                let w = (size.0 * d.size as f32).round().min(tiles.w);
                (Rect { w, ..tiles }, Rect { x: tiles.x + w, w: INSET, ..tiles })
            }
            DockSide::Right => {
                let w = (size.0 * d.size as f32).round().min(tiles.w);
                let x = tiles.x + tiles.w - w;
                (Rect { x, w, ..tiles }, Rect { x: x - INSET, w: INSET, ..tiles })
            }
            DockSide::Bottom => {
                let h = (size.1 * d.size as f32).round().min(tiles.h);
                let y = tiles.y + tiles.h - h;
                (Rect { y, h, ..tiles }, Rect { y: y - INSET, h: INSET, ..tiles })
            }
        };
        if !d.hidden {
            match d.side {
                DockSide::Left => (tiles.x, tiles.w) = (tiles.x + region.w, tiles.w - region.w),
                DockSide::Right => tiles.w -= region.w,
                DockSide::Bottom => tiles.h -= region.h,
            }
        }
        // inset on every side but the one facing the tiles, whose own inset makes the channel
        let mut r =
            Rect { x: region.x + INSET, y: region.y + INSET, w: region.w - 2. * INSET, h: region.h - 2. * INSET };
        match d.side {
            DockSide::Left => r.w += INSET,
            DockSide::Right => (r.x, r.w) = (r.x - INSET, r.w + INSET),
            DockSide::Bottom => (r.y, r.h) = (r.y - INSET, r.h + INSET),
        }
        let (w, h) = (r.w.max(0.), r.h.max(0.));
        let mut r = Rect { w, h, ..r };
        if let Some(s) = shown {
            // slid toward its own edge by the part not shown yet
            match d.side {
                DockSide::Left => r.x -= (1. - s) * region.w,
                DockSide::Right => r.x += (1. - s) * region.w,
                DockSide::Bottom => r.y += (1. - s) * region.h,
            }
        }
        let handle = (shown.is_none()).then_some(handle);
        let size = match d.side {
            DockSide::Bottom => region.h / size.1.max(1.),
            _ => region.w / size.0.max(1.),
        } as f64;
        docks.push(Dock { block_id: d.block_id.clone(), side: d.side, size, rect: r, handle, leaving: d.hidden });
    }
    let floats = o
        .floating
        .iter()
        .filter(|f| !f.hidden)
        .map(|f| {
            let b = f.bounds;
            let (x, y) = ((b.x as f32 * size.0).round(), (b.y as f32 * size.1).round());
            let w = (b.width as f32 * size.0).round().min(size.0 - x);
            let h = (b.height as f32 * size.1).round().min(size.1 - y);
            Float { block_id: f.block_id.clone(), bounds: b, rect: Rect { x, y, w, h } }
        })
        .collect();
    Placed { tiles, docks, floats }
}

/// The block input goes to: a focused overlay even over a zoomed tile, else the zoomed tile, else
/// the focused one (the model keeps zoom and focus apart, so this is what the user sees).
pub fn target(w: &Window) -> Option<&str> {
    let overlay = w.focus.as_deref().filter(|id| !w.layout.as_ref().is_some_and(|t| t.contains(id)));
    overlay.or(w.zoomed.as_deref()).or(w.focus.as_deref())
}

/// The smallest a float's edge drag makes it, in pixels (w, h).
pub const MIN_FLOAT: (f32, f32) = (160., 80.);

/// Which edges of a float a drag moves.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Edges {
    pub left: bool,
    pub right: bool,
    pub top: bool,
    pub bottom: bool,
}

/// What a drag started on, with the placement then and the pane area's size.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Grab {
    Move { bounds: Bounds, area: (f32, f32) },
    Resize { bounds: Bounds, edges: Edges, area: (f32, f32) },
    Dock { side: DockSide, size: f64, area: (f32, f32) },
}

/// Where a drag puts its float or drawer: what `session.float_block` or `session.dock_block` gets.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Placement {
    Float(Bounds),
    Dock(f64),
}

impl Grab {
    /// The placement after the pointer moved (dx, dy) pixels from where the drag started.
    pub fn moved(&self, dx: f32, dy: f32) -> Placement {
        match *self {
            Grab::Move { bounds: b, area } => {
                let x = (b.x + dx as f64 / area.0 as f64).clamp(0., (1. - b.width).max(0.));
                let y = (b.y + dy as f64 / area.1 as f64).clamp(0., (1. - b.height).max(0.));
                Placement::Float(inside(Bounds { x, y, ..b }))
            }
            Grab::Resize { bounds: b, edges, area } => {
                let (fx, fy) = (dx as f64 / area.0 as f64, dy as f64 / area.1 as f64);
                // never below the minimum, unless it was already smaller
                let min_w = (MIN_FLOAT.0 as f64 / area.0 as f64).min(b.width);
                let min_h = (MIN_FLOAT.1 as f64 / area.1 as f64).min(b.height);
                let (x, w) = edge(b.x, b.width, fx, min_w, edges.left, edges.right);
                let (y, h) = edge(b.y, b.height, fy, min_h, edges.top, edges.bottom);
                Placement::Float(inside(Bounds { x, y, width: w, height: h }))
            }
            Grab::Dock { side, size, area } => {
                let by = match side {
                    DockSide::Left => dx as f64 / area.0 as f64,
                    DockSide::Right => -dx as f64 / area.0 as f64,
                    DockSide::Bottom => -dy as f64 / area.1 as f64,
                };
                // a size set elsewhere (0.02 from the CLI) stays until the drag moves it
                Placement::Dock((size + by).clamp(size.min(0.05), size.max(0.95)))
            }
        }
    }
}

/// One axis of a resize: (start, length) after moving the low and/or high edge by `by`.
fn edge(start: f64, len: f64, by: f64, min: f64, low: bool, high: bool) -> (f64, f64) {
    let end = start + len;
    let start2 = if low { (start + by).clamp(0., end - min) } else { start };
    let end2 = if high { (end + by).clamp(start2 + min, 1.) } else { end };
    (start2, end2 - start2)
}

/// `b` with any sum that rounded past 1 trimmed off its size (the server checks x + width <= 1;
/// `x + (1 - x)` never rounds past 1).
fn inside(mut b: Bounds) -> Bounds {
    if b.x + b.width > 1. {
        b.width = 1. - b.x;
    }
    if b.y + b.height > 1. {
        b.height = 1. - b.y;
    }
    b
}

/// How long a drawer slides in or out.
pub const SLIDE: Duration = Duration::from_millis(120);

/// How far into a slide its eased part done is `done` (0–1): what `slide` inverts, so a slide
/// turned back halfway carries on from where it was.
pub fn slide_at(done: f32) -> Duration {
    SLIDE.mul_f32(1. - (1. - done.clamp(0., 1.)).cbrt())
}

/// A slide `elapsed` into it: the part done, eased out (fast, then settling); None once it is
/// over, or at once with Reduce Motion.
pub fn slide(elapsed: Duration, reduce_motion: bool) -> Option<f32> {
    if reduce_motion || elapsed >= SLIDE {
        return None;
    }
    let t = elapsed.as_secs_f32() / SLIDE.as_secs_f32();
    Some(1. - (1. - t).powi(3))
}
