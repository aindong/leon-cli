//! The panes moving when the layout changes (motion PR 2, `layout_motion`): a split, a close, a
//! zoom or an unzoom moves them from where they are to where they go; a tab or session switch fades
//! in under a veil. Only a change of which panes show or of the zoom moves them; a ratio or the
//! area changing snaps. Worked out once a frame, after `prepare`, where the window gives the pane
//! area; render, `pane_rects` (the headers' title cut, the hover) and `tile_order` read the result.

use std::collections::HashMap;

use super::*;
use crate::layout_motion::{self as lm, Ease, LAYOUT, SWITCH};
use crate::panes::Rect;

/// A window's key (its session and tab), its tiles' area, and its tiles in leaf order.
type Target = ((String, String), Rect, Vec<(String, Rect)>);

#[derive(Default)]
pub(super) struct LayoutUi {
    /// The session and tab the tiles were last laid out for, the tiles' area then, and which panes
    /// showed and which was zoomed.
    seen: Option<(String, String)>,
    area: Rect,
    shape: (Vec<String>, Option<String>),
    /// Where each pane rests: the last layout no motion was heading for. Its text sizes are the
    /// PTYs' sizes, kept while panes move, so a program gets one SIGWINCH, when its motion ends.
    settled: HashMap<String, Rect>,
    /// The layout the panes are heading for, in leaf order.
    target: Vec<(String, Rect)>,
    motion: Option<Motion>,
    /// This frame's tiles in drawing order (a zoom's or unzoom's pane last).
    frame: Vec<(String, Rect)>,
    /// The veil a switch fades in under: when it started, and from how opaque.
    veil: Option<(Instant, f32)>,
}

struct Motion {
    from: HashMap<String, Rect>,
    to: HashMap<String, Rect>,
    at: Instant,
    ease: Ease,
    /// Drawn last: the pane a zoom grows or an unzoom shrinks.
    top: Option<String>,
    /// Drawn though out of the layout, their grids frozen: a closing pane (shrinking away) or the
    /// panes a zoom covers (still), until the motion ends.
    ghosts: Vec<String>,
}

impl Workspace {
    /// After `prepare`, on every notify of the workspace: this frame's layout, then the grids and
    /// sections it shows (a closing pane's grid kept, the headers' widths).
    pub(super) fn sync_views(&mut self, seen: &grid_view::Screens, window: &Window, cx: &mut Context<Self>) {
        self.note_layout(window);
        self.sync_grids(seen, cx);
        self.sync_sections(window, cx);
    }

    /// Lay the active window's tiles out for this frame: start a motion when which panes show or the
    /// zoom changed (from where they are drawn now, so a second change mid-motion carries on), snap
    /// for a ratio or the area, fade in a switch; then where each tile is now.
    pub(super) fn note_layout(&mut self, window: &Window) {
        let now = self.motion_now();
        let Some((key, area, target)) = self.layout_target(window) else {
            self.layout = LayoutUi::default();
            return;
        };
        let shape = (target.iter().map(|(id, _)| id.clone()).collect(), self.active_window().and_then(|w| w.zoomed));
        let l = &self.layout;
        let (seen_before, same_place) = (l.seen.is_some(), l.seen.as_ref() == Some(&key));
        let changed_shape = same_place && shape != l.shape;
        let moved = same_place && target != l.target;
        if !same_place {
            // another tab or session (or the first look): the tiles rest at once; a switch fades in,
            // from where a veil still fading is
            let veil = self.veil_now(now);
            let reduce = seen_before && self.reduce_motion();
            let l = &mut self.layout;
            l.veil = (seen_before && !reduce).then_some((now, if veil > 0. { veil } else { 1. }));
            (l.motion, l.settled) = (None, target.iter().cloned().collect());
        } else if changed_shape && l.area == area && !self.reduce_motion() {
            // a change that moves nothing (a lone pane zoomed) rests at once
            let motion = Some(self.motion_to(&target, &shape, now)).filter(|m| !m.ghosts.is_empty() || m.from != m.to);
            if motion.is_none() {
                self.layout.settled = target.iter().cloned().collect();
            }
            self.layout.motion = motion;
        } else if moved {
            (self.layout.motion, self.layout.settled) = (None, target.iter().cloned().collect());
        }
        let l = &mut self.layout;
        (l.seen, l.area, l.shape, l.target) = (Some(key), area, shape, target);
        // where each tile is this frame; a motion over rests
        let done = l
            .motion
            .as_ref()
            .is_none_or(|m| lm::progress(now.saturating_duration_since(m.at), LAYOUT, m.ease).is_none());
        if done && l.motion.take().is_some() {
            l.settled = l.target.iter().cloned().collect();
        }
        l.frame = match &l.motion {
            None => l.target.clone(),
            Some(m) => {
                let p = lm::progress(now.saturating_duration_since(m.at), LAYOUT, m.ease).unwrap_or(1.);
                let at = |id: &String| lm::between(m.from[id], m.to[id], p);
                // the tiles, then what goes or stays covered, then the pane a zoom moves: each once
                // (a view drawn twice in a frame breaks GPUI's reuse of its layouts)
                let tiles = l.target.iter().map(|(id, _)| id).filter(|id| m.top.as_ref() != Some(*id));
                let order = tiles.chain(&m.ghosts).chain(m.top.iter().filter(|t| m.to.contains_key(*t)));
                let mut drawn = Vec::<(String, Rect)>::new();
                for id in order {
                    if !drawn.iter().any(|(d, _)| d == id) {
                        drawn.push((id.clone(), at(id)));
                    }
                }
                drawn
            }
        };
        if self.veil_now(now) == 0. {
            self.layout.veil = None;
        }
    }

    /// The motion from the tiles drawn now to `target`: a new pane grows from the edge of the one
    /// it was split from (or, on an unzoom, shows in place); one gone shrinks into the pane taking
    /// its space (or, covered by a zoom, stays still under it).
    fn motion_to(&self, target: &[(String, Rect)], shape: &(Vec<String>, Option<String>), now: Instant) -> Motion {
        let l = &self.layout;
        let drawn: HashMap<String, Rect> = l.frame.iter().cloned().collect();
        let zoom = shape.1 != l.shape.1;
        let (mut from, mut to): (HashMap<String, Rect>, HashMap<String, Rect>) = (HashMap::new(), HashMap::new());
        for (id, r) in target {
            let start = match drawn.get(id) {
                Some(d) => *d,
                None if zoom => *r,
                None => lm::holding(*r, drawn.iter().map(|(i, d)| (i.as_str(), *d)))
                    .map_or(*r, |(_, h)| lm::collapse(*r, h)),
            };
            from.insert(id.clone(), start);
            to.insert(id.clone(), *r);
        }
        let mut ghosts = Vec::new();
        // a pane a zoom covers is still there: it stays still under it; one closed shrinks away
        let alive: Vec<String> = self
            .active_window()
            .and_then(|w| w.layout.map(|t| t.leaves().into_iter().map(String::from).collect()))
            .unwrap_or_default();
        // a pane still shown out of the tiles (floated, docked) is drawn there, not as a ghost: once
        let shown = self.shown().map(|(_, ids)| ids).unwrap_or_default();
        let gone = |id: &String| !target.iter().any(|(t, _)| t == id) && !shown.contains(id);
        for (id, d) in l.frame.iter().filter(|(id, _)| gone(id)) {
            let end = if zoom && alive.contains(id) {
                *d
            } else {
                lm::holding(*d, target.iter().map(|(i, r)| (i.as_str(), *r))).map_or(*d, |(_, h)| lm::collapse(*d, h))
            };
            ghosts.push(id.clone());
            from.insert(id.clone(), *d);
            to.insert(id.clone(), end);
        }
        // what leaves (a close, an unzoom) eases in; what arrives (a split, a zoom) eases out
        let leaving = target.len() < l.target.len() || (zoom && shape.1.is_none());
        let ease = if leaving { Ease::In } else { Ease::Out };
        Motion { from, to, at: now, ease, top: shape.1.clone().or(l.shape.1.clone()).filter(|_| zoom), ghosts }
    }

    /// The active window's key, its tiles' area, and the tiles `panes::tiles` gives it, in the pane
    /// area's coordinates (as `render_panes` draws them).
    fn layout_target(&self, window: &Window) -> Option<Target> {
        if self.stopped || self.overview_shown() {
            return None;
        }
        let (w, _) = self.shown()?;
        let t = self.placed(&w, self.pane_area(window)).tiles;
        let l = w.layout.as_ref().map(|tree| panes::tiles(tree, w.zoomed.as_deref(), (t.w, t.h))).unwrap_or_default();
        let shift = |r: Rect| Rect { x: r.x + t.x, y: r.y + t.y, ..r };
        let tiles = l.panes.into_iter().map(|p| (p.block_id, shift(p.rect))).collect();
        Some(((self.session_id.clone().unwrap_or_default(), w.id), t, tiles))
    }

    /// The veil's opacity at `now`: 0 once a switch's fade is over.
    fn veil_now(&self, now: Instant) -> f32 {
        let Some((at, from)) = self.layout.veil else { return 0. };
        lm::progress(now.saturating_duration_since(at), SWITCH, Ease::Out).map_or(0., |p| from * (1. - p))
    }

    /// The tiles of window `w` this frame, in drawing order: where they are while they move.
    pub(super) fn layout_tiles(&self, w: &leon_core::Window) -> Option<&[(String, Rect)]> {
        let key = (self.session_id.clone().unwrap_or_default(), w.id.clone());
        (self.layout.seen.as_ref() == Some(&key)).then_some(self.layout.frame.as_slice())
    }

    /// Each pane of window `w` with its rect in the pane area and kind (`pane`, `dock`, `float`,
    /// back to front), as `render_panes` lays them out (a slide moves a drawer, never resizes it).
    pub(super) fn pane_rects(
        &self,
        w: &leon_core::Window,
        window: &Window,
    ) -> Vec<(String, panes::Rect, &'static str)> {
        let placed = self.placed(w, self.pane_area(window));
        let t = placed.tiles;
        let l = w.layout.as_ref().map(|tree| panes::tiles(tree, w.zoomed.as_deref(), (t.w, t.h))).unwrap_or_default();
        let shift = |r: panes::Rect| panes::Rect { x: r.x + t.x, y: r.y + t.y, ..r };
        // where the tiles are this frame, while they move (view_layout.rs)
        let tiles: Vec<_> = match self.layout_tiles(w) {
            Some(frame) => frame.iter().map(|(id, r)| (id.clone(), *r, "pane")).collect(),
            None => l.panes.into_iter().map(|p| (p.block_id, shift(p.rect), "pane")).collect(),
        };
        let docks = placed.docks.into_iter().map(|d| (d.block_id, d.rect, "dock"));
        tiles.into_iter().chain(docks).chain(placed.floats.into_iter().map(|f| (f.block_id, f.rect, "float"))).collect()
    }

    /// Whether a layout motion is on its way (no dividers drawn: they would be at the end's place).
    pub(super) fn layout_moving_panes(&self) -> bool {
        self.layout.motion.is_some()
    }

    /// A pane out of the layout drawn while it goes (closing) or stays covered (a zoom): its tile at
    /// `rect` with its grid as it was, no header (its block may be gone).
    pub(super) fn ghost_tile(&self, id: &str, rect: Rect) -> impl IntoElement {
        let name = format!("pane-{id}");
        let top = panes::PADDING + if self.settings.pane_headers { panes::HEADER } else { 0. };
        div()
            .debug_selector(|| sel(name))
            .absolute()
            .left(px(rect.x))
            .top(px(rect.y))
            .w(px(rect.w))
            .h(px(rect.h))
            .rounded(px(crate::glass::TILE_RADIUS))
            .overflow_hidden()
            .bg(self.pane_fill("pane", &self.pane_theme(id)))
            .child(div().pt(px(top)).pl(px(panes::PADDING)).child(self.grid_element(id)))
    }

    /// Whether panes move (a layout motion or a switch's veil): render asks for frames.
    pub(super) fn layout_moving(&self) -> bool {
        self.layout.motion.is_some() || self.layout.veil.is_some()
    }

    /// The panes drawn though out of the layout (a closing pane, the panes a zoom covers), whose
    /// grids are kept as they were.
    pub(super) fn layout_ghosts(&self) -> &[String] {
        self.layout.motion.as_ref().map_or(&[], |m| m.ghosts.as_slice())
    }

    /// The text size pane `id`'s grid keeps while panes move: its resting one (its PTY's), or a new
    /// pane's final one. None at rest: the grid fills its tile.
    pub(super) fn layout_held(&self, id: &str) -> Option<Rect> {
        self.layout_rest(id).map(|r| panes::text_rect(r, self.settings.pane_headers))
    }

    /// The tile a moving pane is laid out in, clipped by where it is: its resting one (or a new
    /// pane's final one). None for a pane that doesn't move, which keeps its views as they are.
    fn layout_rest(&self, id: &str) -> Option<Rect> {
        let m = self.layout.motion.as_ref().filter(|m| m.from.get(id) != m.to.get(id))?;
        self.layout.settled.get(id).or_else(|| m.to.get(id)).copied()
    }

    /// The width pane `id`'s header is laid out at (its title cut to): its resting tile's while it
    /// moves, so its text is never laid out in a strip a few pixels wide; else its tile's, `w`.
    pub(super) fn header_width(&self, id: &str, w: f32) -> f32 {
        self.layout_rest(id).map_or(w, |r| r.w)
    }

    /// The veil over the panes area this frame (a switch fading in), 0 at rest (tests too).
    pub fn veil(&self) -> f32 {
        self.veil_now(self.motion_now())
    }

    /// The tiles drawn this frame, in drawing order: the last is drawn over the others (tests).
    pub fn tile_order(&self) -> Vec<String> {
        self.layout.frame.iter().map(|(id, _)| id.clone()).collect()
    }
}
