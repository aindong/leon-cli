//! The overview's and the hover previews' thumbnails: a cached view per tab of its panes as laid
//! out, drawn small from the screens the server pushes; which tabs have one; refreshing them at
//! most every `THUMB_EVERY`; and the watch set that brings their screens.

use std::collections::HashMap;

use super::*;

/// The most blocks one server watches (leon-server's `MAX_WATCHED`): a longer list is refused whole.
const MAX_WATCH: usize = 256;
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
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        drawing(cx);
        *self.renders.borrow_mut().entry(self.window_id.clone()).or_default() += 1;
        let mut d = div().size_full().relative().bg(rgb(self.theme.chrome));
        for p in &self.panes {
            let (frame, theme, family, pt) = (p.frame.clone(), self.theme, self.family.clone(), self.font_size);
            let name = format!("card-pane-{}-{}", self.window_id, p.block);
            d = d.child(
                div()
                    .debug_selector(move || sel(name))
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

    /// The tabs that have a thumbnail now, as (host, session, window): the hovered tab, then the
    /// overview's cards from the selected one on, so a capped watch set (`watch_screens`) keeps what
    /// the user is looking at.
    fn thumb_tabs(&self) -> Vec<(usize, String, String)> {
        let mut cards = self.overview_cards();
        let first = self.overview.open.as_ref().map_or(0, |o| o.at(&cards));
        cards.rotate_left(first);
        let hover = self.overview.hover.as_ref().filter(|h| h.shown);
        let hover = hover.map(|h| (h.host, h.session_id.clone(), h.window_id.clone()));
        // only as many as each machine's watch set holds: a tab past it would draw blank anyway,
        // and a server listing thousands of tabs mustn't cost a thumbnail each
        let machines = self.hosts.machines();
        let mut used: HashMap<usize, usize> = HashMap::new();
        used.insert(self.host, self.shown().map_or(0, |(_, ids)| ids.len()));
        let fits = |(host, sid, wid): &(usize, String, String)| {
            let Some(remote) = machines.get(*host).and_then(|m| m.remote()) else { return false };
            let m = remote.model();
            let Some(w) = m.session(sid).and_then(|s| s.windows.iter().find(|w| w.id == *wid)) else { return false };
            let n = if w.zoomed.is_some() { 1 } else { w.layout.as_ref().map_or(0, |t| t.leaves().len()) };
            let u = used.entry(*host).or_default();
            (*u + n <= MAX_WATCH).then(|| *u += n).is_some()
        };
        hover.into_iter().chain(cards.into_iter().map(|c| (c.host, c.session_id, c.window_id))).filter(fits).collect()
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
        (format!("{:?}", self.theme()), &self.font_family, self.font_size.to_bits()).hash(&mut h);
        h.finish()
    }

    /// Make the thumbnails the overview and the preview show, and drop the rest. With `update`
    /// (the poll), refresh one whose tab changed, at most every `THUMB_EVERY` once it has its
    /// screens; render only makes the missing ones.
    pub(in crate::view) fn refresh_thumbs(&mut self, update: bool, cx: &mut Context<Self>) {
        let tabs = self.thumb_tabs();
        if tabs.is_empty() && self.overview.thumbs.is_empty() {
            self.overview.held = None; // nothing left to refresh: no due time for the poll either
            return;
        }
        let look = self.look_stamp();
        let now = Instant::now();
        if update {
            self.overview.held = None;
        }
        self.keep_thumbs(&tabs);
        let mut drew = false;
        for (host, sid, wid) in &tabs {
            let Some((panes, stamp)) = self.thumb_spec(*host, sid, wid, look) else { continue };
            let filled = panes.iter().all(|p| p.frame.is_some());
            let slot = self.overview.thumbs.get(wid);
            let due = slot.is_none_or(|s| update && s.stamp != stamp && (!s.filled || now - s.at >= THUMB_EVERY));
            if !due {
                if let Some(s) = slot.filter(|s| update && s.stamp != stamp) {
                    let at = s.at + THUMB_EVERY;
                    self.overview.held = Some(self.overview.held.map_or(at, |h| h.min(at)));
                }
                continue;
            }
            if slot.is_some() {
                *self.overview.refreshes.entry(wid.clone()).or_default() += 1;
            }
            // a refreshed thumbnail is a new view: it has no cached paint, so it draws, while
            // every other card's view replays its own; nothing rests on GPUI telling a cached
            // view it was notified
            let thumb = Thumb {
                window_id: wid.clone(),
                panes: panes.clone(),
                theme: self.theme(),
                family: self.font_family.clone(),
                font_size: self.font_size,
                renders: self.overview.renders.clone(),
            };
            let view = cx.new(|_| thumb);
            let slot = Slot { view, panes, theme: self.theme(), stamp, at: now, filled };
            self.overview.thumbs.insert(wid.clone(), slot);
            drew |= update;
        }
        // from the poll: the window draws the new views (render makes its own as it goes)
        if drew {
            cx.notify();
        }
    }

    /// Drop the thumbnails of tabs no longer shown: a view kept alive keeps its drawn record.
    fn keep_thumbs(&mut self, tabs: &[(usize, String, String)]) {
        let keep: HashSet<&str> = tabs.iter().map(|t| t.2.as_str()).collect();
        self.overview.thumbs.retain(|w, _| keep.contains(w.as_str()));
        self.overview.refreshes.retain(|w, _| keep.contains(w.as_str()));
        self.overview.renders.borrow_mut().retain(|w, _| keep.contains(w.as_str()));
    }

    /// Before a frame (`prepare`): the overview closed or the hover left, so its thumbnails go.
    pub(in crate::view) fn drop_unshown_thumbs(&mut self) {
        if !self.overview.thumbs.is_empty() {
            let tabs = self.thumb_tabs();
            self.keep_thumbs(&tabs);
        }
    }

    /// How long until a refresh `THUMB_EVERY` held back may run, for the poll's next look.
    pub(in crate::view) fn thumbs_due(&self) -> Option<Duration> {
        self.overview.held.map(|t| t.saturating_duration_since(Instant::now()))
    }

    /// Have every machine push the screens drawn now: `shown` (the active window's panes, on the
    /// selected machine) and each thumbnail's panes, on theirs, up to what a server watches.
    /// shortcut: past `MAX_WATCH` blocks on a machine the last cards have no thumbnail; watch only
    /// the cards scrolled into view if that is ever seen.
    pub(in crate::view) fn watch_screens(&self, shown: &[String]) {
        // a closing overview's cards are on their way out (the moving one keeps its frames)
        let closing = self.overview.open.as_ref().is_some_and(|o| o.closing);
        let tabs = if closing { Vec::new() } else { self.thumb_tabs() };
        for (i, remote) in self.hosts.remotes() {
            let mut ids: Vec<String> = if i == self.host { shown.to_vec() } else { Vec::new() };
            let mut seen: HashSet<String> = ids.iter().cloned().collect();
            let m = remote.model();
            for (_, sid, wid) in tabs.iter().filter(|t| t.0 == i) {
                let Some(w) = m.session(sid).and_then(|s| s.windows.iter().find(|w| w.id == *wid)) else { continue };
                let leaves = match (&w.zoomed, &w.layout) {
                    (Some(z), _) => vec![z.as_str()],
                    (None, Some(t)) => t.leaves(),
                    (None, None) => vec![],
                };
                ids.extend(leaves.into_iter().filter(|b| seen.insert(b.to_string())).map(String::from));
            }
            ids.truncate(MAX_WATCH); // `thumb_tabs` kept within it; this holds it if a tab grew since
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
