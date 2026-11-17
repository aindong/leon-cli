//! Overlays fading and scaling in and out (motion PR 3, `overlay_motion`): the palette and what it
//! hosts, the find bar, the update notice, a key sequence's hint, and the sidebar's list as it
//! switches. Noted at the end of `prepare` by comparing what shows with what showed (so every way an
//! overlay closes counts, and one closed and opened again in one handler only changes what it
//! shows); a going overlay is drawn from what it last showed. This frame's looks are worked out
//! there, once, and read by render and the sections' inputs.

use std::collections::HashMap;

use super::*;
use crate::overlay_motion::{self as om, Look};
use crate::updates::Notice;

pub(super) use super::overlay::PaletteView;

/// The find bar as drawn: its pane, query, case, regex and count.
pub(super) type FindShown = (String, String, bool, bool, String);

/// A pending sequence and what may follow it.
pub(super) type Hint = (String, Vec<(String, String)>);

#[derive(Default)]
pub(super) struct OverlayUi {
    /// What each overlay showed when last noted (kept while it goes).
    palette: Option<PaletteView>,
    hint: Option<Hint>,
    notice: Option<Notice>,
    find: Option<FindShown>,
    list: Option<SidebarList>,
    /// Each overlay's fade on its way: when it started, and whether showing (else going).
    fades: HashMap<&'static str, (Instant, bool)>,
    /// This frame's looks, by overlay; none for one not drawn.
    looks: HashMap<&'static str, Look>,
    /// Looked at once: nothing fades at launch.
    looked: bool,
}

/// Start overlay `key` showing (`is`) or going from the look it has now, if one is on its way.
fn start(fades: &mut HashMap<&'static str, (Instant, bool)>, key: &'static str, is: bool, t: Instant) {
    let now = fades.get(key).and_then(|(at, showing)| om::look(*showing, t.saturating_duration_since(*at)));
    let from = now.map_or(if is { 0. } else { 1. }, |l| l.opacity);
    let start = t.checked_sub(om::elapsed_at(is, from)).unwrap_or(t);
    fades.insert(key, (start, is));
}

/// Note whether overlay `key` shows (`is`) against whether it did (`was`): a change starts a fade.
fn turn(
    fades: &mut HashMap<&'static str, (Instant, bool)>,
    key: &'static str,
    (was, is): (bool, bool),
    t: Instant,
    moving: bool,
) {
    if was == is {
        return;
    }
    if !moving {
        fades.remove(key);
        return;
    }
    start(fades, key, is, t);
}

impl Workspace {
    /// After `prepare`'s other work: each overlay's fade, what a going one keeps, this frame's looks.
    pub(super) fn note_overlays(&mut self) {
        let t = self.motion_now();
        let moving = self.fade.overlays.looked && !self.reduce_motion();
        let palette = self.palette_open().then(|| self.palette_view());
        let hint = self.pending_hint();
        let notice = self.updates.notice();
        let find = self
            .term
            .find
            .as_ref()
            .map(|f| (f.block.clone(), f.query.clone(), f.case, f.regex, self.find_count().unwrap_or_default()));
        let list = self.places().sidebar;
        let o = &mut self.fade.overlays;
        let live = |key: &str, o: &OverlayUi| o.fades.get(key).is_none_or(|(_, showing)| *showing);
        let was = [
            ("palette", o.palette.is_some() && live("palette", o), palette.is_some()),
            ("hint", o.hint.is_some() && live("hint", o), hint.is_some()),
            ("notice", o.notice.is_some() && live("notice", o), notice.is_some()),
            ("find", o.find.is_some() && live("find", o), find.is_some()),
        ];
        for (key, was, is) in was {
            turn(&mut o.fades, key, (was, is), t, moving);
        }
        // find opened in another pane: the bar there fades in (the one it leaves goes at once)
        let block = |f: &Option<FindShown>| f.as_ref().map(|f| f.0.clone());
        if moving && o.find.is_some() && find.is_some() && block(&o.find) != block(&find) {
            o.fades.insert("find", (t, true));
        }
        // the sidebar switching lists fades the one it switches to in (not the sidebar coming or going)
        if moving && o.list.is_some() && list.is_some() && o.list != list {
            start(&mut o.fades, "list", true, t);
        }
        // a fade over is done; a going overlay's last look goes with it
        o.fades.retain(|_, (at, showing)| om::look(*showing, t.saturating_duration_since(*at)).is_some());
        let going = |key: &str, o: &OverlayUi| o.fades.get(key).is_some_and(|(_, showing)| !*showing);
        let keep = |key, o: &OverlayUi| going(key, o);
        o.palette = palette.or_else(|| o.palette.take().filter(|_| keep("palette", o)));
        o.hint = hint.or_else(|| o.hint.take().filter(|_| keep("hint", o)));
        o.notice = notice.or_else(|| o.notice.take().filter(|_| keep("notice", o)));
        o.find = find.or_else(|| o.find.take().filter(|_| keep("find", o)));
        o.list = list;
        let look = |key: &str, o: &OverlayUi| {
            o.fades
                .get(key)
                .and_then(|(at, showing)| om::look(*showing, t.saturating_duration_since(*at)))
                .unwrap_or(Look::SHOWN)
        };
        let drawn = [
            ("palette", o.palette.is_some()),
            ("hint", o.hint.is_some()),
            ("notice", o.notice.is_some()),
            ("find", o.find.is_some()),
            ("list", o.list.is_some()),
        ];
        o.looks = drawn.into_iter().filter(|(_, d)| *d).map(|(key, _)| (key, look(key, o))).collect();
        o.looked = true;
    }

    /// Whether an overlay fades: render asks for frames.
    pub(super) fn overlays_moving(&self) -> bool {
        !self.fade.overlays.fades.is_empty()
    }

    /// Overlay `name`'s look this frame (`palette`, `hint`, `notice`, `find`, `list`); None when
    /// it isn't drawn (tests too).
    pub fn overlay_look(&self, name: &str) -> Option<Look> {
        self.fade.overlays.looks.get(name).copied()
    }

    /// Whether the palette is going (closed, fading out) (tests).
    pub fn palette_going(&self) -> bool {
        self.fade.overlays.fades.get("palette").is_some_and(|(_, showing)| !*showing)
    }

    /// The palette to draw, its look, and whether it is live (else going: no click handlers).
    pub(super) fn palette_drawn(&self) -> Option<(&PaletteView, Look, bool)> {
        let p = self.fade.overlays.palette.as_ref()?;
        Some((p, self.overlay_look("palette")?, !self.palette_going()))
    }

    /// The hint to draw and its look.
    pub(super) fn hint_drawn(&self) -> Option<(&Hint, Look)> {
        Some((self.fade.overlays.hint.as_ref()?, self.overlay_look("hint")?))
    }

    /// The notice to draw, its look, and whether it is live.
    pub(super) fn notice_drawn(&self) -> Option<(&Notice, Look, bool)> {
        let live = self.fade.overlays.fades.get("notice").is_none_or(|(_, showing)| *showing);
        Some((self.fade.overlays.notice.as_ref()?, self.overlay_look("notice")?, live))
    }

    /// Pane `id`'s find bar to draw (live or going), its look, and whether it is live.
    pub(super) fn find_drawn(&self, id: &str) -> Option<(&FindShown, Look, bool)> {
        let f = self.fade.overlays.find.as_ref().filter(|f| f.0 == id)?;
        let live = self.fade.overlays.fades.get("find").is_none_or(|(_, showing)| *showing);
        Some((f, self.overlay_look("find")?, live))
    }

    /// The sidebar list's look this frame.
    pub(super) fn list_look(&self) -> Look {
        self.overlay_look("list").unwrap_or(Look::SHOWN)
    }
}

/// `el` (`w` wide) as an overlay looks part way: faded, its box `scale` of its width around its
/// centre with its content kept at full width and clipped (GPUI 0.2.2 has no transform on a div,
/// and laying text out at a changing width would reflow it each frame), and `rise` lower.
pub(super) fn presented<E: IntoElement + Styled>(look: Look, w: f32, el: E) -> gpui::AnyElement {
    if look == Look::SHOWN {
        return el.into_any_element();
    }
    let cut = w * (1. - look.scale) / 2.;
    let clip = div().w(px(w - 2. * cut)).overflow_hidden().child(el.ml(px(-cut)));
    // a slot `w` wide, so it stays put however its container anchors it
    div()
        .w(px(w))
        .flex()
        .justify_center()
        .relative()
        .top(px(look.rise))
        .opacity(look.opacity)
        .child(clip)
        .into_any_element()
}

/// `el` faded and lowered as an overlay with no width of its own looks part way (an inline bar, a
/// list): its size stays, so nothing reflows.
pub(super) fn faded<E: Styled>(look: Look, rise: f32, el: E) -> E {
    if look == Look::SHOWN {
        return el;
    }
    el.relative().top(px(rise * look.rise / om::LOWER)).opacity(look.opacity)
}
