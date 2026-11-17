//! Window overlays, owned by the session separately from the tiled tree. Vector order is back
//! to front; showing or focusing a float raises it. Visibility never changes terminal ownership.
use crate::layout::{Direction, Node, Side};
use crate::{ApiError, Session, State};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Bounds {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}
impl Default for Bounds {
    fn default() -> Self {
        Self { x: 0.1, y: 0.1, width: 0.8, height: 0.8 }
    }
}
impl Bounds {
    pub fn check(&self) -> Result<(), ApiError> {
        if [self.x, self.y, self.width, self.height].iter().all(|x| x.is_finite())
            && self.x >= 0.
            && self.y >= 0.
            && self.width > 0.
            && self.height > 0.
            && self.x + self.width <= 1.
            && self.y + self.height <= 1.
        {
            Ok(())
        } else {
            Err(ApiError::bad_request("bounds must be finite, positive in size, and contained in [0,1]"))
        }
    }
}
/// A window as a client draws it, in its pixels (`client.geometry`): what the server needs to work
/// out the cells that client gives a float or drawer, so their program starts at that size. The
/// sums repeat the app's (`floats::place`, `panes::text_rect`, `cells_in`) in the same f32 steps.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Geometry {
    /// The pane area [width, height]: where the tiles, drawers and floats go.
    pub area: [f32; 2],
    /// One cell [width, height].
    pub cell: [f32; 2],
    /// What a float's frame (header, padding) takes from its text [across, down].
    pub float_frame: [f32; 2],
    /// What a drawer's frame and insets take from its share of the area [across, down].
    pub drawer_frame: [f32; 2],
}
impl Geometry {
    pub fn check(&self) -> Result<(), ApiError> {
        let all = [self.area, self.cell, self.float_frame, self.drawer_frame].concat();
        if all.iter().all(|v| v.is_finite() && *v >= 0.) && self.cell.iter().all(|c| *c > 0.) {
            Ok(())
        } else {
            Err(ApiError::bad_request("geometry must be finite and not negative, with a cell bigger than zero"))
        }
    }
    /// The (rows, columns) of a float at `b`.
    pub fn float_cells(&self, b: Bounds) -> (u16, u16) {
        let [aw, ah] = self.area;
        let (x, y) = ((b.x as f32 * aw).round(), (b.y as f32 * ah).round());
        let w = (b.width as f32 * aw).round().min(aw - x);
        let h = (b.height as f32 * ah).round().min(ah - y);
        self.cells(w - self.float_frame[0], h - self.float_frame[1])
    }
    /// The (rows, columns) of a bottom drawer at `size` added after the drawers in `o`, which must
    /// have no bottom one (`drawer_frame` is a bottom drawer's: a side one loses other insets).
    pub fn bottom_drawer_cells(&self, o: &Overlays, size: f64) -> (u16, u16) {
        let [aw, ah] = self.area;
        // the width the visible side drawers before it leave, as `floats::place` takes it
        let tw = o
            .docked
            .iter()
            .filter(|d| !d.hidden && d.side != DockSide::Bottom)
            .fold(aw, |tw, d| tw - (aw * d.size as f32).round().min(tw));
        let h = (ah * size as f32).round().min(ah);
        self.cells(tw - self.drawer_frame[0], h - self.drawer_frame[1])
    }
    /// The cells in a text box of (w, h), at least one row of two (what `Terminal::resize` takes).
    fn cells(&self, w: f32, h: f32) -> (u16, u16) {
        let n = |len: f32, unit: f32, least: u16| ((len.max(0.) / unit).floor().max(0.) as u16).max(least);
        (n(h, self.cell[1], 1), n(w, self.cell[0], 2))
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum DockSide {
    Left,
    Right,
    Bottom,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Floating {
    pub block_id: String,
    pub bounds: Bounds,
    pub hidden: bool,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Docked {
    pub block_id: String,
    pub side: DockSide,
    pub size: f64,
    pub hidden: bool,
}
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Overlays {
    /// Back to front: the last float is on top.
    #[serde(default)]
    pub floating: Vec<Floating>,
    #[serde(default)]
    pub docked: Vec<Docked>,
}
impl Overlays {
    pub fn ids(&self) -> impl Iterator<Item = &str> {
        self.floating.iter().map(|f| f.block_id.as_str()).chain(self.docked.iter().map(|d| d.block_id.as_str()))
    }
    pub fn contains(&self, id: &str) -> bool {
        self.ids().any(|b| b == id)
    }
    pub fn hidden(&self, id: &str) -> bool {
        self.floating.iter().any(|f| f.block_id == id && f.hidden)
            || self.docked.iter().any(|d| d.block_id == id && d.hidden)
    }
    pub fn remove(&mut self, id: &str) {
        self.floating.retain(|f| f.block_id != id);
        self.docked.retain(|f| f.block_id != id);
    }
    pub fn show(&mut self, id: &str) {
        if let Some(i) = self.floating.iter().position(|f| f.block_id == id) {
            let mut f = self.floating.remove(i);
            f.hidden = false;
            self.floating.push(f);
        }
        if let Some(d) = self.docked.iter_mut().find(|d| d.block_id == id) {
            d.hidden = false;
        }
    }
    pub fn check(&self) -> Result<(), ApiError> {
        for f in &self.floating {
            f.bounds.check()?;
        }
        let mut sides = Vec::new();
        for d in &self.docked {
            crate::panes::check_ratio(d.size)?;
            if sides.contains(&d.side) {
                return Err(ApiError::bad_request("a side has more than one dock"));
            }
            sides.push(d.side);
        }
        Ok(())
    }
}
impl Session {
    pub fn overlay(&self, window: &str) -> Overlays {
        self.overlays.get(window).cloned().unwrap_or_default()
    }
    pub(crate) fn detach_placed(&mut self, w: usize, id: &str) {
        self.windows[w].detach(id);
        if let Some(o) = self.overlays.get_mut(&self.windows[w].id) {
            o.remove(id);
        }
        self.repair_focus(w);
    }
    pub(crate) fn repair_focus(&mut self, w: usize) {
        let win = &self.windows[w];
        let o = self.overlay(&win.id);
        if win.focus.as_ref().is_some_and(|id| !o.hidden(id)) {
            return;
        }
        self.windows[w].focus = o
            .floating
            .iter()
            .rev()
            .find(|f| !f.hidden)
            .map(|f| f.block_id.clone())
            .or_else(|| o.docked.iter().find(|d| !d.hidden).map(|d| d.block_id.clone()))
            .or_else(|| self.windows[w].layout.as_ref().and_then(|t| t.leaves().first().map(|s| s.to_string())));
    }
    /// Block `id` is shown in window `w`: a tile, or an overlay not hidden.
    fn shown(&self, w: usize, id: &str) -> bool {
        let o = self.overlay(&self.windows[w].id);
        self.windows[w].layout.as_ref().is_some_and(|t| t.contains(id)) || (o.contains(id) && !o.hidden(id))
    }
    /// Overlay `id` has the focus: it is the focused block of the active window.
    pub(crate) fn has_focus(&self, w: usize, id: &str) -> bool {
        self.active == w && self.windows[w].focus.as_deref() == Some(id)
    }
    pub(crate) fn empty_window(&self, w: usize) -> bool {
        self.windows[w].layout.is_none() && self.overlay(&self.windows[w].id).ids().next().is_none()
    }
}
impl State {
    /// When overlay `id` hides (by toggle) or closes while it has the focus, the focus goes back to
    /// `to` if that is still shown. Not saved: after a restart the usual pick applies.
    pub fn set_focus_back(&mut self, id: &str, to: Option<String>) {
        let sessions = &self.sessions;
        self.back_to.retain(|k, _| sessions.iter().any(|s| s.blocks.iter().any(|b| &b.id == k)));
        match to.filter(|to| to != id) {
            Some(to) => self.back_to.insert(id.into(), to),
            None => self.back_to.remove(id),
        };
    }
    /// After a toggled overlay hid or closed while it had the focus: focus `back`, in its own tab,
    /// if it is still shown; else the focus stays where the usual pick put it.
    pub(crate) fn focus_back(&mut self, sid: &str, back: Option<String>) {
        let Some(back) = back else { return };
        let Ok(s) = self.session(sid) else { return };
        if s.window_of(&back).is_ok_and(|w| s.shown(w, &back)) {
            let _ = self.focus_block(sid, &back);
        }
    }
    /// Showing a hidden overlay any way but a toggle (the switcher, `leon float`) forgets where a
    /// toggle would give the focus back.
    pub(crate) fn forget_back_if_hidden(&mut self, sid: &str, id: &str) {
        let hidden =
            self.session(sid).is_ok_and(|s| s.window_of(id).is_ok_and(|w| s.overlay(&s.windows[w].id).hidden(id)));
        if hidden {
            self.back_to.remove(id);
        }
    }
    pub fn float_block(&mut self, sid: &str, id: &str, bounds: Bounds) -> Result<u64, ApiError> {
        bounds.check()?;
        self.forget_back_if_hidden(sid, id);
        let s = self.session_mut(sid)?;
        let w = s.window_of(id)?;
        s.detach_placed(w, id);
        s.overlays.entry(s.windows[w].id.clone()).or_default().floating.push(Floating {
            block_id: id.into(),
            bounds,
            hidden: false,
        });
        s.windows[w].focus = Some(id.into());
        s.windows[w].zoomed = None;
        self.focus_block(sid, id)?;
        self.touch(sid);
        Ok(self.revision(sid))
    }
    pub fn dock_block(&mut self, sid: &str, id: &str, side: DockSide, size: f64) -> Result<u64, ApiError> {
        crate::panes::check_ratio(size)?;
        let s = self.session(sid)?;
        let w = s.window_of(id)?;
        if s.overlay(&s.windows[w].id).docked.iter().any(|d| d.side == side && d.block_id != id) {
            return Err(ApiError::bad_request("that side already has a dock"));
        }
        self.forget_back_if_hidden(sid, id);
        let s = self.session_mut(sid)?;
        // a resize of a drawer on its own side keeps its place: the list order decides which
        // drawer spans the corner
        let wid = s.windows[w].id.clone();
        match s.overlays.get_mut(&wid).and_then(|o| o.docked.iter_mut().find(|d| d.block_id == id && d.side == side)) {
            Some(d) => (d.size, d.hidden) = (size, false),
            None => {
                s.detach_placed(w, id);
                s.overlays.entry(wid).or_default().docked.push(Docked {
                    block_id: id.into(),
                    side,
                    size,
                    hidden: false,
                });
            }
        }
        s.windows[w].focus = Some(id.into());
        s.windows[w].zoomed = None;
        self.focus_block(sid, id)?;
        self.touch(sid);
        Ok(self.revision(sid))
    }
    pub fn unfloat_block(&mut self, sid: &str, id: &str) -> Result<u64, ApiError> {
        self.back_to.remove(id);
        let s = self.session_mut(sid)?;
        let w = s.window_of(id)?;
        if !s.overlay(&s.windows[w].id).contains(id) {
            return Err(ApiError::bad_request("block is already tiled"));
        }
        if let Some(t) = &s.windows[w].layout {
            t.check_insert(t.id(), None)?;
        }
        s.detach_placed(w, id);
        match &mut s.windows[w].layout {
            None => s.windows[w].layout = Some(Node::block(id)),
            Some(t) => {
                let anchor = t.id().to_string();
                t.insert(&anchor, id, Direction::Horizontal, Side::After, 0.5);
            }
        }
        s.windows[w].focus = Some(id.into());
        s.windows[w].zoomed = None;
        self.focus_block(sid, id)?;
        self.touch(sid);
        Ok(self.revision(sid))
    }
    /// Exact, case-insensitive label; never interpret a label as a position or ID suffix.
    pub fn overlay_by_label(&self, sid: &str, label: &str) -> Result<Option<String>, ApiError> {
        let label =
            crate::model::clean_label(Some(label))?.ok_or_else(|| ApiError::bad_request("label must not be empty"))?;
        let s = self.session(sid)?;
        let matches: Vec<_> =
            s.blocks.iter().filter(|b| b.label.as_ref().is_some_and(|l| l.eq_ignore_ascii_case(&label))).collect();
        match matches.as_slice() {
            [] => Ok(None),
            [b] if s.overlay(&b.window_id).contains(&b.id) => Ok(Some(b.id.clone())),
            [_] => Err(ApiError::bad_request("label belongs to a tiled block")),
            _ => Err(ApiError::bad_request("ambiguous block label")),
        }
    }
    pub fn toggle_overlay(&mut self, sid: &str, label: &str) -> Result<u64, ApiError> {
        let id = self
            .overlay_by_label(sid, label)?
            .ok_or_else(|| ApiError::not_found("no float or drawer with that label"))?;
        let back = self.back_to.get(&id).cloned();
        let s = self.session_mut(sid)?;
        let w = s.window_of(&id)?;
        let had_focus = s.has_focus(w, &id);
        // where the key was pressed: the active tab's pane, which may be in another tab
        let before = s.windows[s.active].focus.clone();
        let o = s.overlays.get_mut(&s.windows[w].id).expect("overlay found");
        let hidden = !o.hidden(&id);
        if hidden {
            for f in &mut o.floating {
                if f.block_id == id {
                    f.hidden = true;
                }
            }
            for d in &mut o.docked {
                if d.block_id == id {
                    d.hidden = true;
                }
            }
            if s.windows[w].zoomed.as_deref() == Some(&id) {
                s.windows[w].zoomed = None;
            }
            s.repair_focus(w);
        } else {
            o.show(&id);
            s.windows[w].focus = Some(id.clone());
            s.windows[w].zoomed = None;
        }
        if !hidden {
            self.focus_block(sid, &id)?;
            self.set_focus_back(&id, before);
        } else if had_focus {
            self.focus_back(sid, back);
        }
        self.touch(sid);
        Ok(self.revision(sid))
    }
}
