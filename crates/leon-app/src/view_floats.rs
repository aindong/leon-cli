//! Floats and drawers (M20b): drawn over and beside the tiles from the window's overlays
//! (`floats::place`), moved and resized by dragging (each step `session.float_block` or
//! `session.dock_block`, so the CLI and attach see it), drawers sliding in and out, and the
//! actions that float, unfloat, dock and toggle the scratch drawer.

use std::collections::HashMap;

use gpui::{Div, Point};
use leon_core::overlays::{DockSide, Overlays};

use super::*;
use crate::floats::{self, Edges, Grab, Placed, Placement, SLIDE};
use crate::panes::Rect;

/// How wide a float's edge handles are, half outside it.
const EDGE: f32 = 6.;

#[derive(Default)]
pub(super) struct FloatUi {
    /// Per window, whether each drawer was hidden when last drawn: a change starts a slide.
    seen: HashMap<String, HashMap<String, bool>>,
    /// Drawers sliding: when it started, and whether in (else out).
    slides: HashMap<String, (Instant, bool)>,
    /// Reduce Motion as set by a test; None asks the system.
    reduce_motion: Option<bool>,
    /// What the last frame placed, and where the pane area's corner was in the window.
    placed: RefCell<(Placed, (f32, f32))>,
}

impl Workspace {
    /// Use (or ignore) Reduce Motion whatever the system says (tests).
    pub fn set_reduce_motion(&mut self, on: bool) {
        self.floats.reduce_motion = Some(on);
    }

    /// Whether a drawer is sliding: true until the frame after its slide ended has been drawn (the
    /// poll redraws while this holds, so a slide's last frame is its end).
    pub fn sliding(&self) -> bool {
        !self.floats.slides.is_empty()
    }

    pub(super) fn reduce_motion(&self) -> bool {
        self.floats.reduce_motion.unwrap_or_else(system_reduce_motion)
    }

    /// The active window's overlays, as the app last heard them.
    pub(super) fn overlays_of(&self, w: &leon_core::Window) -> Overlays {
        let m = self.app.model();
        self.session_id.as_deref().and_then(|sid| m.session(sid)).map(|s| s.overlay(&w.id)).unwrap_or_default()
    }

    /// Whether block `id` is a float or a drawer.
    pub(super) fn is_overlay(&self, id: &str) -> bool {
        let m = self.app.model();
        m.block(id).is_some_and(|(s, b)| s.overlays.get(&b.window_id).is_some_and(|o| o.contains(id)))
    }

    /// Start a slide for each drawer of the active window that was shown or hidden since the last
    /// frame (not for a window seen the first time: opening the app slides nothing), and forget
    /// the slides that are over.
    pub(super) fn note_slides(&mut self) {
        self.floats.slides.retain(|_, (at, _)| at.elapsed() < SLIDE);
        let Some(w) = self.active_window() else { return };
        let now: HashMap<String, bool> =
            self.overlays_of(&w).docked.iter().map(|d| (d.block_id.clone(), d.hidden)).collect();
        if let Some(before) = self.floats.seen.get(&w.id) {
            let changed: Vec<(String, bool)> = now
                .iter()
                .filter(|(id, hidden)| before.get(*id).copied().unwrap_or(true) != **hidden)
                .map(|(id, hidden)| (id.clone(), !hidden))
                .collect();
            // the system is asked only when something would slide
            if !changed.is_empty() && !self.reduce_motion() {
                for (id, coming) in changed {
                    // turned back mid-slide: carry on from the part shown now
                    let shown = self.slide_of(&id).unwrap_or(if coming { 0. } else { 1. });
                    let done = if coming { shown } else { 1. - shown };
                    let start = Instant::now().checked_sub(floats::slide_at(done)).unwrap_or_else(Instant::now);
                    self.floats.slides.insert(id, (start, coming));
                }
            }
        }
        self.floats.seen.insert(w.id.clone(), now);
    }

    /// A drawer's shown part while it slides (rising in, falling out).
    fn slide_of(&self, id: &str) -> Option<f32> {
        let (at, coming) = self.floats.slides.get(id)?;
        floats::slide(at.elapsed(), false).map(|s| if *coming { s } else { 1. - s })
    }

    /// The active window's overlays placed in a pane area of `size`.
    pub(super) fn placed(&self, w: &leon_core::Window, size: (f32, f32)) -> Placed {
        floats::place(&self.overlays_of(w), size, &|id| self.slide_of(id))
    }

    /// The blocks the active window shows over or beside its tiles: visible floats and drawers,
    /// and drawers still sliding out.
    pub(super) fn overlay_ids(&self, w: &leon_core::Window) -> Vec<String> {
        let o = self.overlays_of(w);
        let floats = o.floating.iter().filter(|f| !f.hidden).map(|f| &f.block_id);
        let docks = o.docked.iter().filter(|d| !d.hidden || self.slide_of(&d.block_id).is_some()).map(|d| &d.block_id);
        floats.chain(docks).cloned().collect()
    }

    /// Draw the drawers, then the floats back to front, over the tiles already in `out`.
    pub(super) fn render_overlays(
        &self,
        mut out: Div,
        placed: Placed,
        terms: &Panes,
        tiling: &Tiling,
        (area, origin): ((f32, f32), (f32, f32)),
        cx: &mut Context<Self>,
    ) -> Div {
        for d in placed.docks.iter().filter(|d| terms.contains(&d.block_id)) {
            let tile = self.pane_tile("dock", &d.block_id, d.rect, None, tiling, cx);
            out = out.child(if d.leaving { tile } else { tile.occlude() });
            let Some(h) = d.handle else { continue };
            let grab = Grab::Dock { side: d.side, size: d.size, area };
            let across = d.side != DockSide::Bottom;
            out = out.child(
                handle(format!("dock-handle-{}", d.block_id), h)
                    .cursor(if across { CursorStyle::ResizeLeftRight } else { CursorStyle::ResizeUpDown })
                    .on_mouse_down(MouseButton::Left, self.grab_listener(&d.block_id, grab, cx)),
            );
        }
        for f in placed.floats.iter().filter(|f| terms.contains(&f.block_id)) {
            let focused = tiling.target.as_deref() == Some(f.block_id.as_str());
            let t = self.pane_theme(&f.block_id);
            let mv = Grab::Move { bounds: f.bounds, area };
            out = out.child(
                self.pane_tile("float", &f.block_id, f.rect, Some(mv), tiling, cx)
                    .occlude()
                    .border_1()
                    .border_color(rgb(if focused { t.accent } else { t.border }))
                    .shadow_lg(),
            );
            for (name, edges, r, cursor) in edge_handles(f.rect) {
                let grab = Grab::Resize { bounds: f.bounds, edges, area };
                out = out.child(
                    handle(format!("float-edge-{name}-{}", f.block_id), r)
                        .cursor(cursor)
                        .on_mouse_down(MouseButton::Left, self.grab_listener(&f.block_id, grab, cx)),
                );
            }
        }
        *self.floats.placed.borrow_mut() = (placed, origin);
        out
    }

    /// A mouse-down that starts dragging `grab` on block `id` (and focuses it, as a click does).
    pub(super) fn grab_listener(
        &self,
        id: &str,
        grab: Grab,
        cx: &mut Context<Self>,
    ) -> impl Fn(&gpui::MouseDownEvent, &mut Window, &mut App) + 'static {
        let (id, ws) = (id.to_string(), cx.entity().downgrade());
        move |ev: &gpui::MouseDownEvent, _: &mut Window, cx: &mut App| {
            ws.update(cx, |ws, cx| {
                ws.drag = Some(Drag::Overlay { block: id.clone(), grab, from: ev.position, last: None });
                ws.focus_block(&id, cx);
                cx.stop_propagation();
            })
            .ok();
        }
    }

    /// A float or drawer follows the pointer: the placement the drag makes, sent when it changed.
    pub(super) fn drag_overlay(&mut self, at: Point<Pixels>, cx: &mut Context<Self>) {
        let Some(Drag::Overlay { block, grab, .. }) = &self.drag else { return };
        // placed elsewhere meanwhile (the CLI, a key): the drag would undo that, so it ends
        let o = self.app.model().block(block).and_then(|(s, b)| s.overlays.get(&b.window_id).cloned());
        let still = o.is_some_and(|o| match grab {
            Grab::Dock { side, .. } => o.docked.iter().any(|d| d.block_id == *block && d.side == *side && !d.hidden),
            _ => o.floating.iter().any(|f| f.block_id == *block && !f.hidden),
        });
        if !still {
            self.drag = None;
            return;
        }
        let Some(Drag::Overlay { block, grab, from, last }) = self.drag.as_mut() else { return };
        let moved = at - *from;
        let p = grab.moved(f32::from(moved.x), f32::from(moved.y));
        if *last == Some(p) {
            return; // each call is a layout change with events for every subscriber
        }
        *last = Some(p);
        let (block, grab) = (block.clone(), *grab);
        match (p, grab) {
            (Placement::Float(bounds), _) => {
                self.call("session.float_block", json!({ "block_id": block, "bounds": bounds }), cx)
            }
            (Placement::Dock(size), Grab::Dock { side, .. }) => {
                self.call("session.dock_block", json!({ "block_id": block, "side": side, "size": size }), cx)
            }
            _ => {}
        }
    }

    /// The pane whose painted grid is under `at` (window coordinates): an overlay's when one is on
    /// top there, else whichever grid holds the point.
    pub(super) fn grid_at(&self, at: Point<Pixels>, painted: &scroll::PaintedMap) -> Option<String> {
        let (placed, (ox, oy)) = &*self.floats.placed.borrow();
        let on_top = placed.hit(f32::from(at.x) - ox, f32::from(at.y) - oy).map(String::from);
        let painted = painted.borrow();
        let inside = |p: &scroll::Painted| {
            let (w, h) = (p.cell.0 * p.size.1 as f32, p.cell.1 * p.size.0 as f32);
            Bounds::new(p.origin, size(w, h)).contains(&at)
        };
        match on_top {
            Some(id) => painted.get(&id).filter(|p| inside(p)).map(|_| id),
            None => painted.iter().find(|(_, p)| inside(p)).map(|(id, _)| id.clone()),
        }
    }

    /// `pane.float`, `pane.new_floating`, `pane.unfloat`, `pane.scratch.toggle`, `pane.dock.*`.
    pub(super) fn overlay_action(&mut self, name: &str, cx: &mut Context<Self>) {
        let target = self.target().map(|(_, id, _)| id);
        let overlay = target.as_deref().is_some_and(|id| self.is_overlay(id));
        match (name, target) {
            ("pane.new_floating", _) => self.call("session.new_floating", json!({ "layout": self.here() }), cx),
            ("pane.scratch.toggle", _) => {
                self.call("session.toggle", json!({ "label": "scratch", "create": true }), cx)
            }
            ("pane.float", Some(id)) if !self.is_float(&id) => {
                self.call("session.float_block", json!({ "block_id": id }), cx)
            }
            ("pane.unfloat", Some(id)) if overlay => self.call("session.unfloat_block", json!({ "block_id": id }), cx),
            (dock, Some(id)) if dock.starts_with("pane.dock.") => {
                let side = &dock["pane.dock.".len()..];
                self.call("session.dock_block", json!({ "block_id": id, "side": side }), cx)
            }
            _ => {}
        }
    }

    /// Whether block `id` is a hidden float or drawer (still drawn while it slides out).
    pub(super) fn hidden_overlay(&self, id: &str) -> bool {
        let m = self.app.model();
        m.block(id).is_some_and(|(s, b)| s.overlays.get(&b.window_id).is_some_and(|o| o.hidden(id)))
    }

    fn is_float(&self, id: &str) -> bool {
        let m = self.app.model();
        let o = m.block(id).and_then(|(s, b)| s.overlays.get(&b.window_id));
        o.is_some_and(|o| o.floating.iter().any(|f| f.block_id == id))
    }

    /// Every block of window `w` with its terminal's area, as it is or would be in a pane area of
    /// `area`: the tiles (the zoomed one filling their part; a header when `headers`), and every
    /// float and drawer, hidden ones too, so a font change sizes them all before they are shown.
    /// A float or drawer always has its header, and a float its border.
    pub(super) fn window_rects(
        s: &crate::remote::Session,
        w: &leon_core::Window,
        area: (f32, f32),
        headers: bool,
    ) -> Vec<(String, Rect)> {
        let o = s.overlay(&w.id);
        let placed = floats::place(&o, area, &|_| None);
        let (tx, ty) = (placed.tiles.x, placed.tiles.y);
        let mut out = Vec::new();
        if let Some(tree) = &w.layout {
            let size = (placed.tiles.w, placed.tiles.h);
            let zoomed = w.zoomed.as_deref().map(|z| panes::tiles(tree, Some(z), size).panes);
            for p in panes::tiles(tree, None, size).panes {
                let r = zoomed.iter().flatten().find(|z| z.block_id == p.block_id).map_or(p.rect, |z| z.rect);
                out.push((p.block_id, panes::text_rect(Rect { x: r.x + tx, y: r.y + ty, ..r }, headers)));
            }
        }
        let mut all = o;
        all.floating.iter_mut().for_each(|f| f.hidden = false);
        all.docked.iter_mut().for_each(|d| d.hidden = false);
        let shown = floats::place(&all, area, &|_| None);
        out.extend(shown.docks.into_iter().map(|d| (d.block_id, panes::text_rect(d.rect, true))));
        for f in shown.floats {
            let r = f.rect;
            let inside = Rect { x: r.x + 1., y: r.y + 1., w: (r.w - 2.).max(0.), h: (r.h - 2.).max(0.) };
            out.push((f.block_id, panes::text_rect(inside, true)));
        }
        out
    }
}

/// An invisible drag handle over `r`.
fn handle(name: String, r: Rect) -> gpui::Stateful<Div> {
    div()
        .id(SharedString::from(name.clone()))
        .debug_selector(|| sel(name))
        .absolute()
        .left(px(r.x))
        .top(px(r.y))
        .w(px(r.w))
        .h(px(r.h))
        .occlude()
}

/// A float's edge and corner handles: (name, edges moved, where, cursor). Corners come last, so
/// they win where they overlap an edge.
fn edge_handles(r: Rect) -> Vec<(&'static str, Edges, Rect, CursorStyle)> {
    let (h, c) = (EDGE / 2., EDGE * 2.);
    let e = |left, right, top, bottom| Edges { left, right, top, bottom };
    let (x0, x1, y0, y1) = (r.x, r.x + r.w, r.y, r.y + r.h);
    vec![
        (
            "left",
            e(true, false, false, false),
            Rect { x: x0 - h, y: y0, w: EDGE, h: r.h },
            CursorStyle::ResizeLeftRight,
        ),
        (
            "right",
            e(false, true, false, false),
            Rect { x: x1 - h, y: y0, w: EDGE, h: r.h },
            CursorStyle::ResizeLeftRight,
        ),
        ("top", e(false, false, true, false), Rect { x: x0, y: y0 - h, w: r.w, h: EDGE }, CursorStyle::ResizeUpDown),
        ("bottom", e(false, false, false, true), Rect { x: x0, y: y1 - h, w: r.w, h: EDGE }, CursorStyle::ResizeUpDown),
        (
            "top-left",
            e(true, false, true, false),
            Rect { x: x0 - h, y: y0 - h, w: c, h: c },
            CursorStyle::ResizeUpLeftDownRight,
        ),
        (
            "top-right",
            e(false, true, true, false),
            Rect { x: x1 - c + h, y: y0 - h, w: c, h: c },
            CursorStyle::ResizeUpRightDownLeft,
        ),
        (
            "bottom-left",
            e(true, false, false, true),
            Rect { x: x0 - h, y: y1 - c + h, w: c, h: c },
            CursorStyle::ResizeUpRightDownLeft,
        ),
        (
            "bottom-right",
            e(false, true, false, true),
            Rect { x: x1 - c + h, y: y1 - c + h, w: c, h: c },
            CursorStyle::ResizeUpLeftDownRight,
        ),
    ]
}

/// System Settings › Accessibility › Display › Reduce motion.
fn system_reduce_motion() -> bool {
    use objc2::msg_send;
    use objc2::runtime::{AnyClass, AnyObject, Bool};
    let Some(cls) = AnyClass::get(c"NSWorkspace") else { return false };
    unsafe {
        let ws: *mut AnyObject = msg_send![cls, sharedWorkspace];
        if ws.is_null() {
            return false;
        }
        let on: Bool = msg_send![ws, accessibilityDisplayShouldReduceMotion];
        on.as_bool()
    }
}
