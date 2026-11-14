//! Attention in the window: the Activity list (⌘⇧A, the toolbar button) of every block that needs
//! the user across every machine, the blocked count on that button and the Dock, notifications
//! for what changed, and clicks on them.

use super::switch::with_models;
use super::*;
use crate::attention::{self, Row};
use crate::notify;

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
        for c in notify::take_clicks() {
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
                (&r.block_id, r.status, &r.place, &r.caption, attention::waited(r.since, now)).hash(h);
            }
        }
    }

    /// The toolbar's Activity button, with the blocked count when there is one.
    pub(super) fn activity_button(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = &DARK;
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
    pub(super) fn render_activity(&self, cx: &mut Context<Self>) -> gpui::Div {
        let t = &DARK;
        let (frame, now) = (self.pulse(), Instant::now());
        let rows = self.attention.rows();
        let mut list =
            div().debug_selector(|| sel("activity".into())).flex().flex_col().gap(px(1.)).px(px(8.)).pt(px(4.)).child(
                div().px(px(10.)).h(px(24.)).text_size(px(SECONDARY_TEXT)).text_color(rgb(t.muted)).child("Activity"),
            );
        if rows.is_empty() {
            list = list.child(div().px(px(10.)).py(px(6.)).text_color(rgb(t.muted)).child("Nothing needs you"));
        }
        for (i, r) in rows.into_iter().enumerate() {
            let what = match &r.caption {
                Some(c) => format!("{} · {c}", attention::state_text(r.status)),
                None => attention::state_text(r.status),
            };
            let (host, sid, block) = (r.host, r.session_id.clone(), r.block_id.clone());
            list = list.child(
                div()
                    .id(SharedString::from(format!("activity-row-{i}")))
                    .debug_selector(move || sel(format!("activity-row-{i}")))
                    .flex()
                    .flex_none()
                    .items_center()
                    .gap(px(8.))
                    .px(px(10.))
                    .py(px(5.))
                    .rounded(px(6.))
                    .cursor_pointer()
                    .hover(|d| d.bg(rgb(t.hover)))
                    .child(status_mark(r.status, frame))
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .flex()
                            .flex_col()
                            .child(div().line_clamp(1).text_ellipsis().text_color(rgb(t.title)).child(r.place))
                            .child(
                                div()
                                    .line_clamp(1)
                                    .text_ellipsis()
                                    .text_size(px(SECONDARY_TEXT))
                                    .text_color(rgb(t.muted))
                                    .child(what),
                            ),
                    )
                    .child(
                        div()
                            .flex_none()
                            .text_size(px(SECONDARY_TEXT))
                            .text_color(rgb(t.muted))
                            .child(attention::waited(r.since, now)),
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
