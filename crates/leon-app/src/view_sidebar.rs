//! The sidebar: the toolbar row, then each session as a quiet label with its tabs as rows under it
//! (sidebar style), or one row per session (top style, the tabs then in the titlebar). Once there
//! are hosts, sessions are grouped under their machine's label, This Mac first. Its right edge
//! drags to resize it.

use super::*;

/// The sidebar's tab rows and sessions-only rows, and a session label's height.
const ROW: f32 = 30.;
const LABEL: f32 = 24.;

impl Workspace {
    pub(super) fn render_sidebar(
        &self,
        rows: &[SessionRow],
        list: SidebarList,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let t = &self.theme;
        let frame = self.pulse();
        let mut items = div().flex().flex_col().gap(px(1.)).px(px(8.)).pt(px(4.));
        let machines = self.hosts.machines();
        // with hosts, each machine's label goes before its sessions (rows come in machine order)
        let activity = list == SidebarList::Activity;
        let grouped = self.grouped() && !activity;
        let mut labelled = 0;
        for (i, row) in rows.iter().enumerate().filter(|_| !activity) {
            while grouped && labelled <= row.host {
                items = items.child(self.host_label(labelled, &machines[labelled], cx));
                labelled += 1;
            }
            if grouped && self.host_collapsed(&machines[row.host].label) {
                continue;
            }
            let selected = self.session_id.as_deref() == Some(row.id.as_str());
            if list == SidebarList::SessionsOnly {
                items = items.child(self.session_row(i, row, selected, frame, cx));
                continue;
            }
            let open = !self.collapsed.contains(&row.id);
            items = items.child(self.session_label(i, row, open, frame, cx));
            if !open {
                continue;
            }
            for (w, win) in row.windows.iter().enumerate() {
                let current = selected && w == row.active;
                let sid = row.id.clone();
                let hovered = (row.host, row.id.clone(), win.id.clone());
                items =
                    items.child(
                        div()
                            .id(SharedString::from(format!("row-{i}-{w}")))
                            .debug_selector(|| sel(format!("row-{i}-{w}")))
                            .on_hover(cx.listener(move |ws, on: &bool, window, cx| {
                                ws.hover_tab(*on, hovered.clone(), true, window, cx)
                            }))
                            .flex()
                            .flex_none()
                            .items_center()
                            .gap(px(8.))
                            .h(px(ROW))
                            .px(px(10.))
                            .rounded(px(6.))
                            .cursor_pointer()
                            .when(current, |d| d.bg(rgb(t.selected)).text_color(rgb(t.title)))
                            .when(!current, |d| d.text_color(rgb(t.text)).hover(|d| d.bg(rgb(t.hover))))
                            .child(
                                div()
                                    .flex_1()
                                    .overflow_hidden()
                                    .min_w(px(0.))
                                    .line_clamp(1)
                                    .text_ellipsis()
                                    .child(win.title.clone()),
                            )
                            .children(win.status.map(|s| {
                                status_mark(s, frame, t).debug_selector(|| sel(format!("row-status-{i}-{w}")))
                            }))
                            .when(selected && w < 9, |d| {
                                d.child(
                                    div()
                                        .text_size(px(SECONDARY_TEXT))
                                        .text_color(rgb(t.muted))
                                        .child(format!("⌘{}", w + 1)),
                                )
                            })
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |ws, _, _, cx| ws.select(&sid, Some(w), cx)),
                            ),
                    );
            }
            items = items.child(div().h(px(6.)));
        }
        while grouped && labelled < machines.len() {
            items = items.child(self.host_label(labelled, &machines[labelled], cx));
            labelled += 1;
        }
        if activity {
            items = self.render_activity(cx);
        }
        let width = self.settings.sidebar_width;
        div()
            .id("sidebar")
            .debug_selector(|| sel("sidebar".into()))
            .relative()
            .flex()
            .flex_col()
            .flex_none()
            .w(px(width))
            .h_full()
            .bg(self.chrome_fill())
            .child(self.render_toolbar(cx))
            .child(div().id("sidebar-list").flex_1().overflow_y_scroll().child(items))
            .child(
                div()
                    .id("sidebar-edge")
                    .debug_selector(|| sel("sidebar-edge".into()))
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .right_0()
                    .w(px(6.))
                    .cursor(CursorStyle::ResizeLeftRight)
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |ws, ev: &gpui::MouseDownEvent, _, cx| {
                            ws.drag = Some(Drag::Sidebar { from: ev.position.x, width: ws.settings.sidebar_width });
                            cx.stop_propagation();
                            cx.notify();
                        }),
                    ),
            )
    }

    /// A session in sidebar style: a small muted label. Clicking it selects the session, or, when
    /// it is selected, collapses or expands it; collapsed, it shows the session's status.
    fn session_label(
        &self,
        i: usize,
        row: &SessionRow,
        open: bool,
        frame: u64,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let t = &self.theme;
        let (id, sid) = (row.id.clone(), row.id.clone());
        div()
            .id(SharedString::from(format!("session-{i}")))
            .debug_selector(|| sel(format!("session-{i}")))
            .group(SharedString::from(format!("session-{i}")))
            .flex()
            .flex_none()
            .items_center()
            .gap(px(6.))
            .h(px(LABEL))
            .mt(px(4.))
            .px(px(10.))
            .cursor_pointer()
            .text_size(px(SECONDARY_TEXT))
            .font_weight(FontWeight::MEDIUM)
            .text_color(rgb(t.muted))
            .child(
                div().flex_1().overflow_hidden().min_w(px(0.)).line_clamp(1).text_ellipsis().child(row.title.clone()),
            )
            .when(!open, |d| {
                d.children(
                    row.status.map(|s| status_mark(s, frame, t).debug_selector(|| sel(format!("session-status-{i}")))),
                )
            })
            .child(
                div()
                    .id(SharedString::from(format!("session-new-tab-{i}")))
                    .group(SharedString::from(format!("session-new-tab-{i}")))
                    .px(px(4.))
                    .rounded(px(4.))
                    .invisible()
                    .group_hover(SharedString::from(format!("session-{i}")), |d| d.visible())
                    .hover(|d| d.bg(rgb(t.hover)).text_color(rgb(t.title)))
                    .child(Icon::Plus.in_button(ROW_ICON, t.muted, t.title, format!("session-new-tab-{i}")))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |ws, _, window, cx| {
                            cx.stop_propagation();
                            ws.select(&sid, None, cx);
                            ws.new_tab(&NewTab, window, cx);
                        }),
                    ),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |ws, _, _, cx| {
                    let collapsed = ws.collapsed.contains(&id);
                    if ws.session_id.as_deref() == Some(id.as_str()) && !collapsed {
                        ws.collapsed.insert(id.clone());
                    } else {
                        ws.collapsed.remove(&id);
                    }
                    ws.select(&id, None, cx);
                }),
            )
    }

    /// A session in top style: one row; its status shows while its tabs are not on screen.
    fn session_row(
        &self,
        i: usize,
        row: &SessionRow,
        selected: bool,
        frame: u64,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let t = &self.theme;
        let id = row.id.clone();
        div()
            .id(SharedString::from(format!("session-{i}")))
            .debug_selector(|| sel(format!("session-{i}")))
            .flex()
            .flex_none()
            .items_center()
            .gap(px(8.))
            .h(px(ROW))
            .px(px(10.))
            .rounded(px(6.))
            .cursor_pointer()
            .when(selected, |d| d.bg(rgb(t.selected)).text_color(rgb(t.title)))
            .when(!selected, |d| d.text_color(rgb(t.text)).hover(|d| d.bg(rgb(t.hover))))
            .child(
                div().flex_1().overflow_hidden().min_w(px(0.)).line_clamp(1).text_ellipsis().child(row.title.clone()),
            )
            .when(!selected, |d| {
                d.children(
                    row.status.map(|s| status_mark(s, frame, t).debug_selector(|| sel(format!("session-status-{i}")))),
                )
            })
            .on_mouse_down(MouseButton::Left, cx.listener(move |ws, _, _, cx| ws.select(&id, None, cx)))
    }
}
