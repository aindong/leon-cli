//! The workspace's chrome: the sessions sidebar and the tab bar.

use super::*;

impl Workspace {
    pub(super) fn render_sidebar(&self, rows: &[SessionRow], cx: &mut Context<Self>) -> impl IntoElement {
        let t = &DARK;
        let mut list = div().flex().flex_col().gap(px(2.)).px(px(8.));
        for (i, row) in rows.iter().enumerate() {
            let selected = self.session_id.as_deref() == Some(row.id.as_str());
            let open = !self.collapsed.contains(&row.id);
            let (id, toggle_id) = (row.id.clone(), row.id.clone());
            let header = div()
                .id(SharedString::from(format!("session-{i}")))
                .flex()
                .items_center()
                .gap(px(6.))
                .h(px(28.))
                .px(px(6.))
                .rounded(px(6.))
                .cursor_pointer()
                .when(selected, |d| d.bg(rgb(t.hover)).text_color(rgb(0xffffff)))
                .when(!selected, |d| d.text_color(rgb(t.text)).hover(|d| d.bg(rgb(t.hover))))
                .child(
                    div()
                        .id(SharedString::from(format!("chevron-{i}")))
                        .w(px(12.))
                        .text_color(rgb(t.muted))
                        .child(if open { "▾" } else { "▸" })
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |ws, _, _, cx| {
                                if !ws.collapsed.remove(&toggle_id) {
                                    ws.collapsed.insert(toggle_id.clone());
                                }
                                cx.stop_propagation();
                                cx.notify();
                            }),
                        ),
                )
                .child(div().flex_1().overflow_hidden().text_ellipsis().child(row.title.clone()))
                .child(
                    div()
                        .text_xs()
                        .px(px(6.))
                        .rounded(px(8.))
                        .bg(rgb(t.border))
                        .text_color(rgb(t.muted))
                        .child(row.windows.len().to_string()),
                )
                .on_mouse_down(MouseButton::Left, cx.listener(move |ws, _, _, cx| ws.select(&id, None, cx)));
            list = list.child(header);
            if open {
                for (w, win) in row.windows.iter().enumerate() {
                    let active = w == row.active;
                    let sid = row.id.clone();
                    list = list.child(
                        div()
                            .id(SharedString::from(format!("window-{i}-{w}")))
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .h(px(24.))
                            .pl(px(26.))
                            .pr(px(6.))
                            .rounded(px(6.))
                            .text_sm()
                            .cursor_pointer()
                            .text_color(rgb(if active && selected { t.text } else { t.muted }))
                            .hover(|d| d.bg(rgb(t.hover)))
                            .child(div().size(px(6.)).rounded_full().bg(rgb(if active { t.accent } else { t.border })))
                            .child(div().flex_1().overflow_hidden().text_ellipsis().child(win.title.clone()))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |ws, _, _, cx| ws.select(&sid, Some(w), cx)),
                            ),
                    );
                }
            }
        }
        div()
            .flex()
            .flex_col()
            .w(px(SIDEBAR_WIDTH))
            .h_full()
            .flex_none()
            .bg(rgb(t.chrome))
            .border_r_1()
            .border_color(rgb(t.border))
            .child(
                div()
                    .mt(px(TAB_BAR_HEIGHT)) // the traffic lights' row
                    .h(px(28.))
                    .flex()
                    .items_center()
                    .justify_between()
                    .px(px(14.))
                    .text_xs()
                    .text_color(rgb(t.muted))
                    .child("SESSIONS")
                    .child(
                        div()
                            .id("new-session")
                            .px(px(6.))
                            .rounded(px(4.))
                            .cursor_pointer()
                            .hover(|d| d.bg(rgb(t.hover)).text_color(rgb(t.text)))
                            .child("+")
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|ws, _, window, cx| ws.new_session(&NewSession, window, cx)),
                            ),
                    ),
            )
            .child(div().flex_1().child(list))
            .child(
                div().p(px(14.)).text_xs().whitespace_nowrap().text_color(rgb(t.muted)).child("⌘N session  ·  ⌘T tab"),
            )
    }

    pub(super) fn render_tabs(&self, row: Option<&SessionRow>, cx: &mut Context<Self>) -> impl IntoElement {
        let t = &DARK;
        let mut bar = div()
            .flex()
            .flex_none()
            .items_end()
            .h(px(TAB_BAR_HEIGHT))
            .bg(rgb(t.chrome))
            .border_b_1()
            .border_color(rgb(t.border))
            .px(px(6.))
            .when(!self.sidebar, |d| d.pl(px(78.))) // clear the traffic lights (after px, which sets left too)
            .gap(px(2.));
        if let Some(row) = row {
            for (i, w) in row.windows.iter().enumerate() {
                let active = i == row.active;
                let (sid, wid) = (row.id.clone(), w.id.clone());
                bar = bar.child(
                    div()
                        .id(SharedString::from(format!("tab-{i}")))
                        .group("tab")
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .h(px(30.))
                        .px(px(12.))
                        .min_w(px(120.))
                        .max_w(px(220.))
                        .rounded_t(px(7.))
                        .text_sm()
                        .cursor_pointer()
                        .when(active, |d| {
                            d.bg(rgb(t.bg)).text_color(rgb(0xffffff)).border_t_2().border_color(rgb(t.accent))
                        })
                        .when(!active, |d| {
                            d.text_color(rgb(t.muted)).hover(|d| d.bg(rgb(t.hover)).text_color(rgb(t.text)))
                        })
                        .child(div().flex_1().overflow_hidden().text_ellipsis().child(w.title.clone()))
                        .when(i < 9, |d| d.child(div().text_xs().text_color(rgb(t.muted)).child(format!("⌘{}", i + 1))))
                        .child(
                            div()
                                .id(SharedString::from(format!("close-{i}")))
                                .px(px(4.))
                                .rounded(px(4.))
                                .text_color(rgb(t.muted))
                                .invisible()
                                .group_hover("tab", |d| d.visible())
                                .hover(|d| d.bg(rgb(t.border)).text_color(rgb(t.text)))
                                .child("×")
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(move |ws, _, _, cx| {
                                        ws.close_window((sid.clone(), wid.clone()), cx);
                                        cx.stop_propagation();
                                    }),
                                ),
                        )
                        .on_mouse_down(MouseButton::Left, cx.listener(move |ws, _, _, cx| ws.focus_window(i, cx))),
                );
            }
        }
        bar.child(
            div()
                .id("new-tab")
                .flex()
                .items_center()
                .justify_center()
                .size(px(28.))
                .mb(px(1.))
                .rounded(px(6.))
                .cursor_pointer()
                .text_color(rgb(t.muted))
                .hover(|d| d.bg(rgb(t.hover)).text_color(rgb(t.text)))
                .child("+")
                .on_mouse_down(MouseButton::Left, cx.listener(|ws, _, window, cx| ws.new_tab(&NewTab, window, cx))),
        )
    }
}
