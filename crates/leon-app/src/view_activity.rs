//! Attention in the window: the Activity list (⌘⇧A, the toolbar button) of every block that needs
//! the user across every machine, the blocked count on that button and the Dock, notifications
//! for what changed, and clicks on them.

use super::switch::with_models;
use super::*;
use crate::attention::{self, Row};
use crate::notify;

/// The width an Activity row's text loses to the list's and the row's padding, the status mark
/// and the waited time (`12s`), with a little to spare.
const ACTIVITY_INSET: f32 = 96.;

impl Workspace {
    pub fn activity_visible(&self) -> bool {
        self.activity
    }

    /// The Activity list as of the last poll, in order.
    pub fn activity_rows(&self) -> Vec<Row> {
        self.attention.rows()
    }

    /// The Activity button's count: how many blocks are blocked, or None.
    pub fn activity_badge(&self) -> Option<String> {
        attention::badge(self.attention.blocked())
    }

    /// The Dock badge as last set.
    pub fn dock_badge(&self) -> Option<String> {
        self.dock.clone()
    }

    /// The poll's share: go where a clicked notification points, look at every machine's status,
    /// post what is news, and keep the Dock badge on the blocked count.
    pub(super) fn poll_attention(&mut self, cx: &mut Context<Self>) {
        // a click is this window's when one of its machines has the pane or session (a storm's
        // notice names no pane: any window shows Activity)
        let hosts = &self.hosts;
        let knows = |c: &notify::Click| {
            let remote = hosts.index_of(&c.host_label).and_then(|i| hosts.machines()[i].remote().cloned());
            let m =
                remote.map(|r| r.model().block(&c.block_id).is_some() || r.model().session(&c.session_id).is_some());
            c.block_id.is_empty() || m == Some(true)
        };
        for c in notify::take_clicks(knows) {
            match self.hosts.index_of(&c.host_label).filter(|_| !c.block_id.is_empty()) {
                Some(host) => self.go_to(host, &c.session_id, None, Some(&c.block_id), cx),
                None => self.activity = true, // the notice for a storm: show what needs you
            }
            cx.notify();
        }
        let now = Instant::now();
        let tracker = &mut self.attention;
        let changes = with_models(&self.hosts, |models| tracker.update(models, now));
        let shown = self.shown().map(|(_, ids)| ids).unwrap_or_default();
        self.notifier.settings = self.keymap.notify;
        let mut notices: Vec<_> = changes
            .iter()
            .filter_map(|t| {
                let on_screen = t.host == self.host && shown.contains(&t.block_id);
                self.notifier.offer(t, now, self.window_active, on_screen)
            })
            .collect();
        notices.extend(self.notifier.flush(now));
        if self.platform {
            notices.iter().for_each(notify::post);
        }
        let badge = self.activity_badge();
        if badge != self.dock {
            if self.platform {
                notify::set_badge(badge.as_deref());
            }
            self.dock = badge;
        }
    }

    /// What the Activity list and the badge draw, for the redraw check.
    pub(super) fn activity_signature(&self, h: &mut DefaultHasher) {
        (self.activity, self.attention.blocked()).hash(h);
        if self.activity {
            let now = Instant::now();
            for r in self.attention.rows() {
                (&r.block_id, r.status, &r.place, &r.title, &r.caption, attention::waited(r.since, now)).hash(h);
            }
        }
    }

    /// The toolbar's Activity button, with the blocked count when there is one.
    pub(super) fn activity_button(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = &self.theme;
        div()
            .relative()
            .child(self.tool("toolbar-activity", Icon::Blocked, "client.sidebar.activity.toggle", cx))
            .children(self.activity_badge().map(|n| {
                div()
                    .debug_selector(|| sel("toolbar-badge".into()))
                    .absolute()
                    .top(px(1.))
                    .right(px(0.))
                    .px(px(4.))
                    .rounded(px(7.))
                    .bg(rgb(t.blocked))
                    .text_color(rgb(t.chrome))
                    .text_size(px(10.))
                    .font_weight(FontWeight::BOLD)
                    .child(n)
            }))
    }

    /// The Activity list: blocked, error, done, working, each with where it is, what it says and
    /// how long it has waited; a click goes to its pane (which clears a done or error there).
    /// From the sidebar's input: the rows with their waited times, the sidebar's width.
    pub(super) fn render_activity(
        &self,
        rows: &[(attention::Row, String)],
        width: f32,
        t: &Theme,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let frame = self.pulse();
        let mut list =
            div().debug_selector(|| sel("activity".into())).flex().flex_col().gap(px(1.)).px(px(8.)).pt(px(4.)).child(
                div().px(px(10.)).h(px(24.)).text_size(px(SECONDARY_TEXT)).text_color(rgb(t.muted)).child("Activity"),
            );
        if rows.is_empty() {
            list = list.child(div().px(px(10.)).py(px(6.)).text_color(rgb(t.muted)).child("Nothing needs you"));
        }
        // GPUI clamps a wrapped line mid-word with no ellipsis, so the second line is cut here, at
        // a word, where the wrapper says a third line would start
        let room = px(width - ACTIVITY_INSET);
        let mut wrapper = cx.text_system().line_wrapper(gpui::font(".SystemUIFont"), px(SECONDARY_TEXT));
        let mut title_wrapper = cx.text_system().line_wrapper(gpui::font(".SystemUIFont"), px(UI_TEXT));
        for (i, (r, waited)) in rows.iter().enumerate() {
            let what = match &r.caption {
                Some(c) => format!("{} · {c}", attention::state_text(r.status)),
                None => attention::state_text(r.status),
            };
            let third = wrapper.wrap_line(&[gpui::LineFragment::text(&what)], room).nth(1).map(|b| b.ix);
            let what = third.map_or(what.clone(), |end| attention::cut_words(&what, end));
            let second = title_wrapper.wrap_line(&[gpui::LineFragment::text(&r.title)], room).next().map(|b| b.ix);
            let title = second.map_or(r.title.clone(), |end| attention::cut_words(&r.title, end));
            // shortcut: a tab title holding " › " splits in the wrong place; display only
            let place = match r.place.rsplit_once(" › ") {
                Some((p, tab)) if tab == r.title => p.to_string(), // the title says the tab already
                _ => r.place.clone(),
            };
            // the progress is in the words below, so the mark stays one icon wide
            let mark = match r.status {
                Indicator::Working { .. } => Indicator::Working { progress: None },
                other => other,
            };
            let (host, sid, block) = (r.host, r.session_id.clone(), r.block_id.clone());
            list = list.child(
                div()
                    .id(SharedString::from(format!("activity-row-{i}")))
                    .debug_selector(move || sel(format!("activity-row-{i}")))
                    .flex()
                    .flex_none()
                    .items_start()
                    .gap(px(8.))
                    .px(px(10.))
                    .py(px(5.))
                    .rounded(px(6.))
                    .cursor_pointer()
                    .hover(|d| d.bg(rgb(t.hover)))
                    .child(div().pt(px(2.)).child(status_mark(mark, frame, t)))
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .flex()
                            .flex_col()
                            .child(div().line_clamp(1).text_ellipsis().text_color(rgb(t.title)).child(title))
                            .child(div().line_clamp(2).text_ellipsis().text_size(px(SECONDARY_TEXT)).child(what))
                            .child(
                                div()
                                    .line_clamp(1)
                                    .text_ellipsis()
                                    .text_size(px(SECONDARY_TEXT))
                                    .text_color(rgb(t.muted))
                                    .child(place),
                            ),
                    )
                    .child(
                        div().flex_none().text_size(px(SECONDARY_TEXT)).text_color(rgb(t.muted)).child(waited.clone()),
                    )
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |ws, _, _, cx| ws.go_to(host, &sid, None, Some(&block), cx)),
                    ),
            );
        }
        list
    }
}
