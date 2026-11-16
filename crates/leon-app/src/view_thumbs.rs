//! The overview's and the hover previews' thumbnails: a cached view per tab of its panes as laid
//! out, drawn small from the screens the server pushes; which tabs have one; refreshing them at
//! most every `THUMB_EVERY`; and the watch set that brings their screens.

use std::collections::HashMap;

use super::*;

/// The least time between two refreshes of one thumbnail: at most 4 a second.
pub const THUMB_EVERY: Duration = Duration::from_millis(250);

/// One tab's thumbnail: the cached view, what it was drawn from, and when it was last refreshed.
pub(super) struct Slot {
    view: gpui::Entity<Thumb>,
    /// What the view was last given, for the tests and the cards' selectors.
    pub(super) panes: Vec<ThumbPane>,
    theme: Theme,
    stamp: u64,
    at: Instant,
    pub(super) filled: bool,
}

/// A tab's panes, as laid out, drawn small from the screens the server pushed.
struct Thumb {
    window_id: String,
    panes: Vec<ThumbPane>,
    theme: Theme,
    family: SharedString,
    font_size: f32,
    renders: Rc<RefCell<HashMap<String, u64>>>,
}

#[derive(Clone)]
pub(super) struct ThumbPane {
    pub(super) block: String,
    /// Its tile as fractions of the tab.
    rect: panes::Rect,
    frame: Option<Arc<render::Frame>>,
}

impl Render for Thumb {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        *self.renders.borrow_mut().entry(self.window_id.clone()).or_default() += 1;
        let mut d = div().size_full().relative().bg(rgb(self.theme.chrome));
        for p in &self.panes {
            let (frame, theme, family, pt) = (p.frame.clone(), self.theme, self.family.clone(), self.font_size);
            let name = format!("card-pane-{}-{}", self.window_id, p.block);
            d = d.child(
                div()
                    .debug_selector(move || name)
                    .absolute()
                    .left(gpui::relative(p.rect.x))
                    .top(gpui::relative(p.rect.y))
                    .w(gpui::relative(p.rect.w))
                    .h(gpui::relative(p.rect.h))
                    .rounded(px(3.))
                    .overflow_hidden()
                    .bg(rgb(theme.bg))
                    .p(px(2.))
                    .child(
                        canvas(
                            |_, _, _| (),
                            move |b, _, window, cx| paint_small(frame.as_deref(), b, &theme, (&family, pt), window, cx),
                        )
                        .size_full(),
                    ),
            );
        }
        d
    }
}

/// `f` scaled down to fit `bounds`: its cells and font shrunk alike, its colours as pushed (never
/// faded), no marks, the cursor as in a window without the focus.
fn paint_small(
    f: Option<&render::Frame>,
    bounds: Bounds<Pixels>,
    t: &Theme,
    (family, pt): (&SharedString, f32),
    window: &mut Window,
    cx: &mut App,
) {
    let Some(f) = f.filter(|f| f.cols > 0 && f.lines > 0) else { return };
    let (cell_w, line_h) = cell_size(window, family, pt);
    let s = (bounds.size.width / (cell_w * f.cols as f32)).min(bounds.size.height / (line_h * f.lines as f32));
    if s <= 0. {
        return;
    }
    window.with_content_mask(Some(gpui::ContentMask { bounds }), |window| {
        paint_frame(f, &[], t, bounds, (cell_w * s, line_h * s), (family, pt * s), false, window, cx)
    });
}

impl Workspace {
    /// The blocks This Mac's server pushes screens of now.
    pub fn watching(&self) -> Vec<String> {
        self.hosts.local().watched()
    }

    /// Whether tab `window_id`'s thumbnail has every pane's screen.
    pub fn thumb_filled(&self, window_id: &str) -> bool {
        self.overview.thumbs.get(window_id).is_some_and(|s| s.filled)
    }

    /// A thumbnail's pane: the screen it draws and the theme it draws it in.
    pub fn thumb_pane(&self, window_id: &str, block: &str) -> Option<(Arc<render::Frame>, Theme)> {
        let slot = self.overview.thumbs.get(window_id)?;
        let p = slot.panes.iter().find(|p| p.block == block)?;
        Some((p.frame.clone()?, slot.theme))
    }

    /// Per tab, how often its thumbnail was refreshed after it was first drawn.
    pub fn thumb_refreshes(&self) -> HashMap<String, u64> {
        self.overview.refreshes.clone()
    }

    /// Per tab, how often its thumbnail's view rendered.
    pub fn thumb_renders(&self) -> HashMap<String, u64> {
        self.overview.renders.borrow().clone()
    }

    /// The tabs that have a thumbnail now: the overview's cards and the hovered tab, as
    /// (host, session, window, greyed).
    fn thumb_tabs(&self) -> Vec<(usize, String, String, bool)> {
        let mut out: Vec<_> =
            self.overview_cards().into_iter().map(|c| (c.host, c.session_id, c.window_id, c.greyed)).collect();
        if let Some(h) = self.overview.hover.as_ref().filter(|h| h.shown) {
            out.push((h.host, h.session_id.clone(), h.window_id.clone(), false));
        }
        out
    }

    /// What tab `wid`'s thumbnail draws now, and a stamp that moves when any of it does.
    fn thumb_spec(&self, host: usize, sid: &str, wid: &str, look: u64) -> Option<(Vec<ThumbPane>, u64)> {
        let remote = self.hosts.machines().get(host)?.remote()?.clone();
        let m = remote.model();
        let w = m.session(sid)?.windows.iter().find(|w| w.id == wid)?;
        let (aw, ah) = (self.overview.area.0.max(1.), self.overview.area.1.max(1.));
        let tiles = w.layout.as_ref().map(|t| panes::tiles(t, w.zoomed.as_deref(), (aw, ah))).unwrap_or_default();
        let mut h = DefaultHasher::new();
        look.hash(&mut h);
        let panes = tiles
            .panes
            .into_iter()
            .map(|p| {
                let r = p.rect;
                (&p.block_id, r.x.to_bits(), r.y.to_bits(), r.w.to_bits(), r.h.to_bits()).hash(&mut h);
                m.screen_updates.get(&p.block_id).hash(&mut h);
                let rect = panes::Rect { x: r.x / aw, y: r.y / ah, w: r.w / aw, h: r.h / ah };
                ThumbPane { frame: m.screens.get(&p.block_id).cloned(), block: p.block_id, rect }
            })
            .collect();
        Some((panes, h.finish()))
    }

    /// The theme and font every thumbnail is drawn with, as a stamp.
    fn look_stamp(&self) -> u64 {
        let mut h = DefaultHasher::new();
        (format!("{:?}", self.theme), &self.font_family, self.font_size.to_bits()).hash(&mut h);
        h.finish()
    }

    /// Make the thumbnails the overview and the preview show, and drop the rest. With `update`
    /// (the poll), refresh one whose tab changed, at most every `THUMB_EVERY` once it has its
    /// screens; render only makes the missing ones.
    pub(in crate::view) fn refresh_thumbs(&mut self, update: bool, cx: &mut Context<Self>) {
        let tabs = self.thumb_tabs();
        if tabs.is_empty() && self.overview.thumbs.is_empty() {
            return;
        }
        let look = self.look_stamp();
        let now = Instant::now();
        let keep: HashSet<&str> = tabs.iter().map(|t| t.2.as_str()).collect();
        self.overview.thumbs.retain(|w, _| keep.contains(w.as_str()));
        for (host, sid, wid, _) in &tabs {
            let Some((panes, stamp)) = self.thumb_spec(*host, sid, wid, look) else { continue };
            let filled = panes.iter().all(|p| p.frame.is_some());
            match self.overview.thumbs.get_mut(wid) {
                None => {
                    let thumb = Thumb {
                        window_id: wid.clone(),
                        panes,
                        theme: self.theme,
                        family: self.font_family.clone(),
                        font_size: self.font_size,
                        renders: self.overview.renders.clone(),
                    };
                    let copy = thumb.panes.clone();
                    let view = cx.new(|_| thumb);
                    let slot = Slot { view, panes: copy, theme: self.theme, stamp, at: now, filled };
                    self.overview.thumbs.insert(wid.clone(), slot);
                }
                Some(slot) if update && slot.stamp != stamp && (!slot.filled || now - slot.at >= THUMB_EVERY) => {
                    let (theme, family, pt) = (self.theme, self.font_family.clone(), self.font_size);
                    (slot.panes, slot.theme) = (panes.clone(), theme);
                    slot.view.update(cx, |t, cx| {
                        (t.panes, t.theme, t.family, t.font_size) = (panes, theme, family, pt);
                        cx.notify();
                    });
                    (slot.stamp, slot.at, slot.filled) = (stamp, now, filled);
                    *self.overview.refreshes.entry(wid.clone()).or_default() += 1;
                }
                Some(_) => {}
            }
        }
    }

    /// Have every machine push the screens drawn now: `shown` (the active window's panes, on the
    /// selected machine) and each thumbnail's panes, on theirs.
    pub(in crate::view) fn watch_screens(&self, shown: &[String]) {
        let tabs = self.thumb_tabs();
        for (i, remote) in self.hosts.remotes() {
            let mut ids: Vec<String> = if i == self.host { shown.to_vec() } else { Vec::new() };
            let m = remote.model();
            for (_, sid, wid, _) in tabs.iter().filter(|t| t.0 == i) {
                let Some(w) = m.session(sid).and_then(|s| s.windows.iter().find(|w| w.id == *wid)) else { continue };
                let leaves = match (&w.zoomed, &w.layout) {
                    (Some(z), _) => vec![z.as_str()],
                    (None, Some(t)) => t.leaves(),
                    (None, None) => vec![],
                };
                ids.extend(
                    leaves.into_iter().filter(|b| !ids.iter().any(|x| x == b)).map(String::from).collect::<Vec<_>>(),
                );
            }
            drop(m);
            remote.watch(ids);
        }
    }

    /// A cached thumbnail of tab `wid`, if it has one.
    pub(super) fn thumb_view(&self, wid: &str) -> Option<gpui::AnyView> {
        let s = self.overview.thumbs.get(wid)?;
        Some(gpui::AnyView::from(s.view.clone()).cached(gpui::StyleRefinement::default().size_full()))
    }
}
