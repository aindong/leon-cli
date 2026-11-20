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
/// How many posted notifications the window keeps a record of.
const POSTED_KEPT: usize = 32;

impl Workspace {
    pub fn activity_visible(&self) -> bool {
        self.activity
    }

    /// The Activity list as of the last model change, in order.
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

    /// The notifications posted, newest last (the last `POSTED_KEPT`; tests).
    pub fn notices_posted(&self) -> Vec<attention::Notice> {
        self.posted.clone()
    }

    /// On a wake: go where a clicked notification points (a click is this window's when one of
    /// its machines has the pane or session; a storm's notice names no pane: any window shows
    /// Activity).
    pub(super) fn take_clicks(&mut self, cx: &mut Context<Self>) {
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
    }

    /// The sessions moved: look at every machine's status, post what is news, keep the Dock
    /// badge on the blocked count, and time the summary of what a storm held back.
    pub(super) fn poll_attention(&mut self, window: &Window, cx: &mut Context<Self>) {
        let now = self.clock_now();
        let tracker = &mut self.attention;
        let changes = with_models(&self.hosts, |models| tracker.update(models, now));
        let shown = self.shown().map(|(_, ids)| ids).unwrap_or_default();
        self.notifier.settings = self.keymap.notify;
        let notices: Vec<_> = changes
            .iter()
            .filter_map(|t| {
                let on_screen = t.host == self.host && shown.contains(&t.block_id);
                self.notifier.offer(t, now, self.window_active, on_screen)
            })
            .collect();
        self.post(notices);
        self.flush_notices(window, cx);
    }

    /// Post the summary of what a storm held back once there is room, and time the next look.
    pub(super) fn flush_notices(&mut self, window: &Window, cx: &mut Context<Self>) {
        let held = self.notifier.flush(self.clock_now());
        self.post(held.into_iter().collect());
        self.arm_notices(window, cx);
    }

    fn post(&mut self, notices: Vec<attention::Notice>) {
        if self.platform {
            notices.iter().for_each(notify::post);
        }
        self.posted.extend(notices);
        let over = self.posted.len().saturating_sub(POSTED_KEPT);
        self.posted.drain(..over);
        let badge = self.activity_badge();
        if badge != self.dock {
            if self.platform {
                notify::set_badge(badge.as_deref());
            }
            self.dock = badge;
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
