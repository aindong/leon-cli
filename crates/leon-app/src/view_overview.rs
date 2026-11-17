//! The overview in the window (`overview.rs` decides what goes where): every tab as a card with a
//! live thumbnail (view_thumbs.rs), the keys while it is open, and its motion in and out. The hover
//! previews of tabs in the titlebar and the sidebar are in view_hover.rs.

use std::collections::HashMap;

use super::*;
use crate::overview::{self, Card, Scope, Step};

#[path = "view_hover.rs"]
mod hover;
#[path = "view_overview_keys.rs"]
mod keys;
#[path = "view_thumbs.rs"]
mod thumbs;
use hover::Hover;
use thumbs::Slot;

/// The overview while it is open, or closing.
pub(super) struct Overview {
    scope: Scope,
    query: String,
    /// The selected card: its tab, and where it was (what's left in its place if the tab goes).
    chosen: Option<String>,
    selected: usize,
    /// Moving in (true) or out, since when; the card that moves is the selected one.
    motion: Option<(bool, Instant)>,
    /// Closing: shown only until its motion ends.
    closing: bool,
    /// Scroll the selected card into view at the next render (on opening, once the grid's size is known).
    reveal: bool,
}

impl Overview {
    /// The selected card's place in `cards`: its tab's, wherever that is now, so a tab closing or
    /// opening before it doesn't move the selection to another tab.
    fn at(&self, cards: &[Card]) -> usize {
        let here = self.chosen.as_ref().and_then(|w| cards.iter().position(|c| c.window_id == *w));
        here.unwrap_or(self.selected).min(cards.len().saturating_sub(1))
    }

    fn choose(&mut self, i: usize, cards: &[Card]) {
        (self.selected, self.chosen) = (i, cards.get(i).map(|c| c.window_id.clone()));
    }
}

#[derive(Default)]
pub(super) struct OverviewUi {
    open: Option<Overview>,
    thumbs: HashMap<String, Slot>,
    /// Per tab: refreshes after the first draw, and renders of its thumbnail (tests).
    refreshes: HashMap<String, u64>,
    /// When the soonest refresh `THUMB_EVERY` held back may run.
    held: Option<Instant>,
    renders: Rc<RefCell<HashMap<String, u64>>>,
    scroll: gpui::ScrollHandle,
    /// The pane area's and the window's size at the last render: what a thumbnail lays its tab
    /// out in, and what the grid fills.
    area: (f32, f32),
    window: (f32, f32),
    hover: Option<Hover>,
    hover_gen: u64,
    /// When a shown preview was last hidden by leaving its tab.
    hover_left: Option<Instant>,
}

impl Workspace {
    pub fn overview_open(&self) -> bool {
        self.overview.open.as_ref().is_some_and(|o| !o.closing)
    }

    /// The open overview's cards as shown (filtered), in order.
    pub fn overview_cards(&self) -> Vec<Card> {
        self.overview.open.as_ref().map(|o| overview::cards(&self.overview_groups(o))).unwrap_or_default()
    }

    pub fn overview_selected(&self) -> usize {
        let cards = self.overview_cards();
        self.overview.open.as_ref().map_or(0, |o| o.at(&cards))
    }

    pub fn overview_query(&self) -> String {
        self.overview.open.as_ref().map(|o| o.query.clone()).unwrap_or_default()
    }

    /// How far the veil over the grid is drawn now (0 once the motion is over).
    pub fn overview_veil(&self) -> f32 {
        self.overview_motion().map_or(0., |(opening, p)| if opening { 1. - p } else { p })
    }

    /// The sidebar's status for tab `window_id` of session `session_id`.
    pub fn tab_status(&self, session_id: &str, window_id: &str) -> Option<Indicator> {
        let rows = self.rows();
        let row = rows.iter().find(|r| r.id == session_id)?;
        row.windows.iter().find(|w| w.id == window_id)?.status
    }

    /// `client.overview.toggle` (`all`) and `client.overview.session.toggle`: open the overview,
    /// or close it, back where it was.
    pub(super) fn toggle_overview(&mut self, action: &str, cx: &mut Context<Self>) {
        let all = action == "client.overview.toggle";
        if self.overview_open() {
            self.close_overview(false, cx);
            return;
        }
        let scope = self.overview_scope(all);
        let motion = (!self.reduce_motion()).then(|| (true, self.motion_now()));
        let mut o =
            Overview { scope, query: String::new(), chosen: None, selected: 0, motion, closing: false, reveal: true };
        let current = self.active_window().map(|w| w.id);
        let cards = overview::cards(&self.overview_groups(&o));
        o.choose(cards.iter().position(|c| Some(&c.window_id) == current.as_ref()).unwrap_or(0), &cards);
        self.overview.open = Some(o);
        self.overview.hover = None;
        self.overview.scroll.set_offset(point(px(0.), px(0.)));
        let stamp = if self.platform { crate::input::current_event_stamp() } else { None };
        self.stats.borrow_mut().overview_opened(stamp);
        cx.notify();
    }

    /// The tabs `client.overview.toggle` (`all`) or `client.overview.session.toggle` shows.
    fn overview_scope(&self, all: bool) -> Scope {
        match (all, self.session_id.clone()) {
            (false, Some(session_id)) => Scope::Session { host: self.host, session_id },
            _ => Scope::All,
        }
    }

    /// Close the overview, going to the selected card's tab when `go`; else the current tab's card
    /// is the one that moves back to the panes.
    fn close_overview(&mut self, go: bool, cx: &mut Context<Self>) {
        let cards = self.overview_cards();
        let current = self.active_window().map(|w| w.id);
        let (reduce, now) = (self.reduce_motion(), self.motion_now());
        let Some(o) = self.overview.open.as_mut() else { return };
        let pick = cards.get(o.at(&cards)).cloned().filter(|_| go);
        let here = cards.iter().position(|c| Some(&c.window_id) == current.as_ref());
        if let Some(i) = here.filter(|_| !go) {
            o.choose(i, &cards);
        }
        // back to a tab the filter hides: no card stands for it, so nothing moves
        let still = reduce || (!go && here.is_none());
        o.closing = true;
        o.motion = (!still).then_some((false, now));
        if still {
            self.overview.open = None;
        }
        if let Some(c) = pick {
            self.go_to(c.host, &c.session_id, Some(&c.window_id), None, cx);
        }
        cx.notify();
    }

    fn overview_groups(&self, o: &Overview) -> Vec<overview::Group> {
        let machines = self.hosts.machines();
        let guards: Vec<_> = machines.iter().map(|m| m.remote().map(|r| r.model())).collect();
        let list: Vec<overview::Machine> = machines
            .iter()
            .enumerate()
            .map(|(i, m)| overview::Machine {
                host: i,
                label: m.label.as_str(),
                model: guards[i].as_deref(),
                note: m.state().note(&m.label),
            })
            .collect();
        overview::filter(&overview::groups(&list, &o.scope, self.grouped()), &o.query)
    }

    /// The motion now: (opening, the part done 0–1), or None when nothing moves.
    fn overview_motion(&self) -> Option<(bool, f32)> {
        let (opening, at) = self.overview.open.as_ref()?.motion?;
        crate::floats::slide(self.motion_now().saturating_duration_since(at), self.reduce_motion())
            .map(|p| (opening, p))
    }

    /// The grid as laid out for the window's width, in the pane area's shape.
    /// `groups` is the one read of the models a frame uses for both: a second read could see a
    /// session or tab more or less, and the layout's indexes would point past the groups.
    fn overview_layout(&self, groups: &[overview::Group]) -> overview::Layout {
        let (w, h) = self.overview.area;
        let (width, height) = self.overview.window;
        overview::fit(groups, width.max(1.), height - TITLEBAR, if w > 0. { h / w } else { 0.6 })
    }

    /// Whether the overview moves: opening or closing (a closing one until `settle_overview` drops
    /// it: its motion can end between a read of the clock and render's).
    pub(super) fn overview_moving(&self) -> bool {
        self.overview_motion().is_some() || self.overview.open.as_ref().is_some_and(|o| o.closing)
    }

    /// Drop an overview whose closing motion is over.
    pub(super) fn settle_overview(&mut self) {
        if self.overview.open.as_ref().is_some_and(|o| o.closing) && self.overview_motion().is_none() {
            self.overview.open = None;
        }
    }

    /// Whether the overview is drawn (open, or closing).
    pub(super) fn overview_shown(&self) -> bool {
        self.overview.open.is_some()
    }

    /// The overview in the whole window: a titlebar row with what is typed, then the groups'
    /// labels and cards; the moving card and the veil over them while it opens or closes.
    pub(super) fn render_overview(&mut self, window: &mut Window, cx: &mut Context<Self>) -> gpui::AnyElement {
        self.overview.area = self.pane_area(window);
        let view = window.viewport_size();
        self.overview.window = (f32::from(view.width), f32::from(view.height));
        self.refresh_thumbs(false, cx);
        let t = self.theme;
        let Some(o) = self.overview.open.as_ref() else { return div().into_any_element() };
        let (query, closing) = (o.query.clone(), o.closing);
        let session = match &o.scope {
            Scope::Session { session_id, .. } => self.rows().into_iter().find(|r| r.id == *session_id).map(|r| r.title),
            Scope::All => None,
        };
        let groups = self.overview_groups(o);
        let cards = overview::cards(&groups);
        let l = self.overview_layout(&groups);
        let (selected, reveal) = (o.at(&cards), o.reveal);
        if reveal {
            // the current tab's card in view on opening: below the fold, scroll down to it
            if let Some(o) = self.overview.open.as_mut() {
                o.reveal = false;
            }
            let h = self.overview.window.1 - TITLEBAR;
            let y = l.cards.get(selected).map_or(0., |r| (r.y + r.h + overview::PAD - h).max(0.));
            self.overview.scroll.set_offset(point(px(0.), px(-y)));
        }
        let motion = self.overview_motion();
        let frame = self.pulse();
        let mut grid = div().relative().w_full().h(px(l.height));
        for (r, gi, host) in &l.labels {
            let g = &groups[*gi];
            let (text, name) = match host {
                true => (g.host_label.clone().unwrap_or_default(), format!("overview-host-{}", g.host)),
                false => (g.session.clone().unwrap_or_default(), format!("overview-session-{gi}")),
            };
            let note = (*host).then(|| g.note.clone()).flatten();
            grid = grid.child(
                div()
                    .debug_selector(move || sel(name))
                    .absolute()
                    .left(px(r.x))
                    .top(px(r.y))
                    .w(px(r.w))
                    .h(px(r.h))
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .text_size(px(SECONDARY_TEXT))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(rgb(t.muted))
                    .child(div().flex_none().max_w(px(r.w)).line_clamp(1).text_ellipsis().child(text))
                    .children(
                        note.map(|n| div().font_weight(FontWeight::NORMAL).line_clamp(1).text_ellipsis().child(n)),
                    ),
            );
        }
        let moving = motion.and_then(|_| cards.get(selected)).map(|c| c.window_id.clone());
        for (i, (c, r)) in cards.iter().zip(&l.cards).enumerate() {
            if moving.as_ref() == Some(&c.window_id) {
                continue; // drawn where it is on its way
            }
            // closing, no card takes a click: the panes are coming back under the pointer
            grid = grid.child(self.card(c, *r, (i == selected, closing), frame, cx));
        }
        if cards.is_empty() {
            grid = grid.child(
                div()
                    .debug_selector(|| sel("overview-empty".into()))
                    .absolute()
                    .left(px(overview::PAD))
                    .top(px(overview::PAD))
                    .text_color(rgb(t.muted))
                    .child("No matching tabs"),
            );
        }
        let title = match &session {
            Some(s) => format!("Overview of {s}"),
            None => "Overview of All Tabs".to_string(),
        };
        let prompt = match query.is_empty() {
            true => div().text_color(rgb(t.muted)).child("Type to filter, ↩ to go, esc to go back"),
            false => div().debug_selector(|| sel("overview-query".into())).text_color(rgb(t.title)).child(query),
        };
        let bar = div()
            .flex()
            .flex_none()
            .items_center()
            .gap(px(12.))
            .h(px(TITLEBAR))
            .pl(px(TRAFFIC_LIGHTS))
            .pr(px(8.))
            .child(div().font_weight(FontWeight::MEDIUM).text_color(rgb(t.title)).child(title))
            .child(prompt)
            .child(div().flex_1())
            .child(self.tool("toolbar-overview", Icon::Grid, "client.overview.toggle", cx));
        let mut out = div()
            .debug_selector(|| sel("overview".into()))
            .size_full()
            .relative()
            .flex()
            .flex_col()
            .bg(rgb(t.chrome))
            .child(bar)
            .child(
                div().id("overview-grid").flex_1().overflow_y_scroll().track_scroll(&self.overview.scroll).child(grid),
            )
            .child(self.stats_mark());
        if let Some((opening, p)) = motion {
            let veil = self.overview_veil();
            // the veil is drawn over the cached thumbnails, never as their opacity: a cached
            // view's paint is replayed as it was recorded
            out = out.child(
                div().absolute().top(px(TITLEBAR)).left_0().right_0().bottom_0().bg(rgb(t.chrome)).opacity(veil),
            );
            if let Some(i) = cards.iter().position(|c| Some(&c.window_id) == moving.as_ref()) {
                let from = self.panes_rect(window);
                let off = f32::from(self.overview.scroll.offset().y);
                let to = l.cards[i];
                let to = panes::Rect { y: to.y + TITLEBAR + off, ..to };
                let k = if opening { p } else { 1. - p };
                let lerp = |a: f32, b: f32| a + (b - a) * k;
                let r = panes::Rect {
                    x: lerp(from.x, to.x),
                    y: lerp(from.y, to.y),
                    w: lerp(from.w, to.w),
                    h: lerp(from.h, to.h),
                };
                // the card itself, header and all, on its way
                let card = self.card(&cards[i], r, (true, true), frame, cx);
                out =
                    out.child(div().debug_selector(|| sel("overview-moving".into())).absolute().inset_0().child(card));
            }
        }
        out.into_any_element()
    }

    /// Where the panes are drawn when the overview is closed, in the window.
    fn panes_rect(&self, window: &Window) -> panes::Rect {
        let (w, h) = self.pane_area(window);
        let view = window.viewport_size();
        panes::Rect { x: f32::from(view.width) - w, y: f32::from(view.height) - h, w, h }
    }

    /// One card: its header (status, title, the state in words), then its thumbnail; a click goes
    /// to its tab.
    fn card(
        &self,
        c: &Card,
        r: panes::Rect,
        (selected, moving): (bool, bool),
        frame: u64,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let t = &self.theme;
        let wid = c.window_id.clone();
        let (name, status_name) = (format!("card-{wid}"), format!("card-status-{wid}"));
        let pick = c.window_id.clone();
        div()
            .id(SharedString::from(name.clone()))
            .debug_selector(move || sel(name))
            .absolute()
            .left(px(r.x))
            .top(px(r.y))
            .w(px(r.w))
            .h(px(r.h))
            .flex()
            .flex_col()
            .rounded(px(6.))
            .overflow_hidden()
            .bg(rgb(t.bg))
            .border_1()
            .border_color(rgb(if selected { t.accent } else { t.border }))
            .cursor_pointer()
            .child(
                div()
                    .flex()
                    .flex_none()
                    .items_center()
                    .gap(px(8.))
                    .h(px(overview::HEADER))
                    .px(px(10.))
                    .text_color(rgb(if selected { t.title } else { t.text }))
                    .children(c.status.map(|s| status_mark(s, frame, t).debug_selector(move || sel(status_name))))
                    .children(c.program.map(|k| program_mark(k, format!("card-program-{wid}"), t.muted)))
                    .child(div().flex_1().min_w(px(0.)).line_clamp(1).text_ellipsis().child(c.title.clone()))
                    .children(c.status.map(|s| {
                        let word = format!("card-word-{}", c.window_id);
                        div()
                            .debug_selector(move || sel(word))
                            .flex_none()
                            .text_size(px(SECONDARY_TEXT))
                            .text_color(rgb(s.color(t)))
                            .child(crate::attention::state_text(s))
                    })),
            )
            .child(div().flex_1().relative().children(self.thumb_view(&c.window_id)))
            // a machine that isn't connected: its last screens, greyed (a veil, as above)
            .when(c.greyed, |d| d.child(div().absolute().size_full().bg(rgb(t.chrome)).opacity(0.55)))
            // a card on its way takes no click: the one under the pointer is what the veil shows
            .when(!moving, |d| {
                d.on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |ws, _, _, cx| {
                        let cards = ws.overview_cards();
                        if let (Some(i), Some(o)) =
                            (cards.iter().position(|c| c.window_id == pick), ws.overview.open.as_mut())
                        {
                            o.choose(i, &cards);
                            ws.close_overview(true, cx);
                        }
                        cx.stop_propagation();
                    }),
                )
            })
    }

    /// A zero-size element whose paint notes an overview frame for `LEON_FRAME_STATS` (the
    /// thumbnails are cached, so their paint doesn't always run); the root's `frame_mark` counts it.
    fn stats_mark(&self) -> impl IntoElement {
        let (stats, filled) = (self.stats.clone(), self.overview.thumbs.values().all(|s| s.filled));
        canvas(
            |_, _, _| (),
            move |_, _, _, _| {
                if stats.borrow().enabled {
                    stats.borrow_mut().overview_drawn(filled);
                }
            },
        )
        .absolute()
        .w(px(0.))
        .h(px(0.))
    }
}
