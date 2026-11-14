//! The toolbar (sidebar toggle, palette, new session) and the titlebar row: the session's name
//! and, in top tab style, its tabs.

use super::*;

impl Workspace {
    /// The toolbar row at the top of the sidebar, beside the traffic lights.
    pub(super) fn render_toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .debug_selector(|| sel("toolbar".into()))
            .flex()
            .flex_none()
            .items_center()
            .gap(px(2.))
            .h(px(TITLEBAR))
            .pl(px(TRAFFIC_LIGHTS))
            .pr(px(8.))
            .child(self.tool("toolbar-sidebar", Icon::Sidebar, "client.sidebar.toggle", cx))
            .child(div().flex_1())
            .child(self.tool("toolbar-palette", Icon::Command, "client.palette.toggle", cx))
            .child(self.tool("toolbar-new", Icon::Plus, "session.new", cx))
    }

    /// One toolbar button: an icon that runs `action`.
    fn tool(&self, id: &'static str, icon: Icon, action: &'static str, cx: &mut Context<Self>) -> impl IntoElement {
        let t = &DARK;
        div()
            .id(id)
            .group(id)
            .debug_selector(|| sel(id.into()))
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .size(px(TOOLBAR_TARGET))
            .tooltip(self.tip(action))
            .rounded(px(6.))
            .cursor_pointer()
            .text_color(rgb(t.muted))
            .hover(|d| d.bg(rgb(t.hover)).text_color(rgb(t.title)))
            .child(icon.in_button(TOOLBAR_ICON, t.muted, t.title, id))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |ws, _, window, cx| {
                    cx.stop_propagation();
                    ws.run_action(action, None, "toolbar", window, cx);
                }),
            )
    }

    /// The titlebar row over the panes: with the sidebar hidden it holds the toolbar's buttons;
    /// then the session's name, and in top style its tabs and a + for a new one.
    pub(super) fn render_titlebar(&self, row: Option<&SessionRow>, cx: &mut Context<Self>) -> impl IntoElement {
        let t = &DARK;
        let places = self.places();
        let frame = self.pulse();
        let hidden = places.sidebar.is_none();
        let mut bar = div()
            .debug_selector(|| sel("titlebar".into()))
            .flex()
            .flex_none()
            .overflow_hidden()
            .items_center()
            .gap(px(4.))
            .h(px(TITLEBAR))
            .pr(px(8.))
            .pl(px(if hidden { TRAFFIC_LIGHTS } else { 10. }))
            .bg(rgb(t.chrome))
            .when(hidden, |d| d.child(self.tool("toolbar-sidebar", Icon::Sidebar, "client.sidebar.toggle", cx)));
        if let Some(row) = row {
            bar = bar.child(
                div()
                    .debug_selector(|| sel("titlebar-session".into()))
                    .flex_none()
                    .px(px(6.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(rgb(t.title))
                    .child(row.title.clone()),
            );
            if places.top_tabs {
                for (i, w) in row.windows.iter().enumerate() {
                    bar = bar.child(self.top_tab(i, w, i == row.active, row, frame, cx));
                }
                bar = bar.child(self.tool("new-tab", Icon::Plus, "client.tab.new", cx));
            } else if let Some(w) = row.windows.get(row.active) {
                // sidebar style with the sidebar hidden: name the tab on screen, draw no tabs
                bar =
                    bar.child(div().min_w(px(0.)).truncate().text_color(rgb(t.muted)).child(format!("› {}", w.title)));
            }
        }
        bar.child(div().flex_1()).when(hidden, |d| {
            d.child(self.tool("toolbar-palette", Icon::Command, "client.palette.toggle", cx)).child(self.tool(
                "toolbar-new",
                Icon::Plus,
                "session.new",
                cx,
            ))
        })
    }

    fn top_tab(
        &self,
        i: usize,
        w: &WindowRow,
        active: bool,
        row: &SessionRow,
        frame: u64,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let t = &DARK;
        let (sid, wid) = (row.id.clone(), w.id.clone());
        div()
            .id(SharedString::from(format!("tab-{i}")))
            .debug_selector(|| sel(format!("tab-{i}")))
            .group(SharedString::from(format!("tab-{i}")))
            .flex()
            .flex_shrink()
            .min_w(px(96.))
            .items_center()
            .gap(px(8.))
            .h(px(26.))
            .pl(px(10.))
            .pr(px(4.))
            .max_w(px(260.))
            .rounded(px(6.))
            .cursor_pointer()
            .when(active, |d| d.bg(rgb(t.selected)).text_color(rgb(t.title)))
            .when(!active, |d| d.text_color(rgb(t.text)).hover(|d| d.bg(rgb(t.hover))))
            .child(
                div()
                    .flex_1()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .min_w(px(0.))
                    .text_ellipsis()
                    .child(w.title.clone()),
            )
            .children(w.status.map(|s| status_mark(s, frame)))
            .when(i < 9, |d| {
                d.child(div().text_size(px(SECONDARY_TEXT)).text_color(rgb(t.muted)).child(format!("⌘{}", i + 1)))
            })
            .child(
                div()
                    .id(SharedString::from(format!("close-{i}")))
                    .group(SharedString::from(format!("close-{i}")))
                    .px(px(4.))
                    .rounded(px(4.))
                    .text_color(rgb(t.muted))
                    .invisible()
                    .group_hover(SharedString::from(format!("tab-{i}")), |d| d.visible())
                    .hover(|d| d.bg(rgb(t.hover)).text_color(rgb(t.title)))
                    .child(Icon::Close.in_button(ROW_ICON, t.muted, t.title, format!("close-{i}")))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |ws, _, _, cx| {
                            ws.close_window((sid.clone(), wid.clone()), cx);
                            cx.stop_propagation();
                        }),
                    ),
            )
            .on_mouse_down(MouseButton::Left, cx.listener(move |ws, _, _, cx| ws.focus_window(i, cx)))
    }
}

/// A tooltip: what a button does, and the keys that do it.
struct Tip(SharedString);

impl Render for Tip {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let t = &DARK;
        div()
            .px(px(8.))
            .py(px(4.))
            .rounded(px(5.))
            .bg(rgb(t.hover))
            .border_1()
            .border_color(rgb(t.border))
            .font_family(".SystemUIFont")
            .text_size(px(SECONDARY_TEXT))
            .text_color(rgb(t.title))
            .child(self.0.clone())
    }
}

impl Workspace {
    /// A tooltip builder for the button that runs `action`: its palette title and live shortcut.
    pub(super) fn tip(&self, action: &str) -> impl Fn(&mut Window, &mut App) -> gpui::AnyView + 'static {
        let title = crate::palette::BUILTINS.iter().find(|(n, _)| *n == action).map_or(action, |(_, t)| *t);
        let text: SharedString = match self.keymap.shortcut(action) {
            Some(keys) => format!("{title}   {keys}").into(),
            None => title.to_string().into(),
        };
        move |_, cx| cx.new(|_| Tip(text.clone())).into()
    }
}

/// A status icon in its state's colour (a tab, a sidebar row, a pane header), with the progress
/// when the program gave one. Working pulses with `frame`.
pub(super) fn status_mark(i: Indicator, frame: u64) -> gpui::Div {
    use leon_core::program_status::Kind;
    const PULSE: [f32; 8] = [1., 0.85, 0.7, 0.55, 0.45, 0.55, 0.7, 0.85];
    let icon = match i {
        Indicator::Working { .. } => Icon::Working,
        Indicator::Blocked(Some(Kind::Permission)) => Icon::Permission,
        Indicator::Blocked(Some(Kind::Question)) => Icon::Question,
        Indicator::Blocked(Some(Kind::Auth)) => Icon::Auth,
        Indicator::Blocked(None) => Icon::Blocked,
        Indicator::Done => Icon::Done,
        Indicator::Error => Icon::Error,
    };
    let progress = match i {
        Indicator::Working { progress: Some(p) } => Some(format!("{p}%")),
        _ => None,
    };
    let pulse = if matches!(i, Indicator::Working { .. }) { PULSE[frame as usize % PULSE.len()] } else { 1. };
    div()
        .flex()
        .flex_none()
        .items_center()
        .gap(px(4.))
        .text_size(px(SECONDARY_TEXT))
        .text_color(rgb(i.color(&DARK)))
        .child(icon.at(ROW_ICON, i.color(&DARK)).opacity(pulse))
        .children(progress)
}
