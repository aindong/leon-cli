//! A pane's header (program, title or cwd, status, caption, and split/zoom/close on the focused or
//! hovered pane), and the empty state shown when there is no session.

use super::*;

impl Workspace {
    pub(super) fn render_header(&self, id: &str, focused: bool, cx: &mut Context<Self>) -> impl IntoElement {
        let t = &DARK;
        let shown = self.app.model().block(id).and_then(|(_, b)| status::indicator_of(&b.newest_first()));
        let caption = self.header_caption(id);
        let frame = self.pulse();
        let title = self.pane_title(id).unwrap_or_default();
        let group = SharedString::from(format!("pane-{id}"));
        let control = |what: &'static str, icon: Icon, action: &'static str, cx: &mut Context<Self>| {
            let (block, name) = (id.to_string(), format!("header-{what}-{id}"));
            div()
                .id(SharedString::from(name.clone()))
                .group(SharedString::from(name.clone()))
                .flex()
                .items_center()
                .justify_center()
                .size(px(22.))
                .rounded(px(4.))
                .cursor_pointer()
                .text_color(rgb(t.muted))
                .hover(|d| d.bg(rgb(t.hover)).text_color(rgb(t.title)))
                .child(icon.in_button(ROW_ICON, t.muted, t.title, name.clone()))
                .debug_selector(|| sel(name))
                .tooltip(self.tip(action))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |ws, _, window, cx| {
                        cx.stop_propagation();
                        ws.focus_block(&block, cx); // the actions act on the focused pane
                        ws.run_action(action, None, "pane_header", window, cx);
                    }),
                )
        };
        let controls = div()
            .flex()
            .flex_none()
            .gap(px(2.))
            .when(!focused, |d| d.invisible().group_hover(group.clone(), |d| d.visible()))
            .child(control("split-right", Icon::SplitRight, "pane.split.right", cx))
            .child(control("split-down", Icon::SplitDown, "pane.split.down", cx))
            .child(control("zoom", Icon::Zoom, "pane.zoom", cx))
            .child(control("close", Icon::Close, "pane.close", cx));
        div()
            .debug_selector(|| sel(format!("header-{id}")))
            .flex()
            .flex_none()
            .items_center()
            .gap(px(6.))
            .h(px(panes::HEADER))
            .pl(px(10.))
            .pr(px(4.))
            .text_size(px(SECONDARY_TEXT))
            .child(div().flex_none().text_color(rgb(t.muted)).child(Icon::Terminal.at(ROW_ICON, t.muted)))
            .child(
                div()
                    .flex_shrink()
                    .max_w(px(320.))
                    .overflow_hidden()
                    .min_w(px(0.))
                    .line_clamp(1)
                    .text_ellipsis()
                    .text_color(rgb(if focused { t.title } else { t.text }))
                    .child(title),
            )
            .children(shown.map(|i| status_mark(i, frame).debug_selector(|| sel(format!("header-status-{id}")))))
            .child(
                div()
                    .debug_selector(|| sel(format!("caption-{id}")))
                    .flex_1()
                    .overflow_hidden()
                    .min_w(px(0.))
                    .line_clamp(1)
                    .text_ellipsis()
                    .text_color(rgb(t.muted))
                    .children(caption),
            )
            .children(self.render_find_bar(id, cx))
            .child(controls)
    }

    /// No session: the name, the tagline, and the three things to know, each one a click away.
    pub(super) fn render_empty(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = &DARK;

        let mut hints = div().flex().flex_col().gap(px(4.)).mt(px(18.));
        for (i, (keys, what, action)) in self.empty_items().into_iter().enumerate() {
            hints = hints.child(
                div()
                    .id(SharedString::from(format!("hint-{i}")))
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .h(px(34.))
                    .px(px(10.))
                    .rounded(px(6.))
                    .when(action.is_some(), |d| d.cursor_pointer().hover(|d| d.bg(rgb(t.hover))))
                    .child(
                        div()
                            .flex_none()
                            .px(px(6.))
                            .rounded(px(4.))
                            .bg(rgb(t.hover))
                            .text_color(rgb(t.accent))
                            .font_family(FONT_FAMILY)
                            .text_size(px(UI_TEXT))
                            .child(keys),
                    )
                    .child(div().text_color(rgb(t.text)).child(what))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |ws, _, window, cx| {
                            if let Some(action) = action {
                                ws.run_action(action, None, "empty_state", window, cx);
                            }
                        }),
                    ),
            );
        }
        div()
            .debug_selector(|| sel("empty-state".into()))
            .flex_1()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .child(mark())
            .child(div().mt(px(16.)).text_size(px(28.)).text_color(rgb(t.title)).child("leon"))
            .child(div().text_color(rgb(t.muted)).child("Keep your work in view."))
            .child(hints)
    }
}

/// leon's icon (the lion), as the Dock shows it.
fn mark() -> impl IntoElement {
    static ICON: std::sync::OnceLock<Arc<gpui::Image>> = std::sync::OnceLock::new();
    let icon = ICON.get_or_init(|| {
        let png = include_bytes!("../../../brand/icon/Leon.iconset/icon_128x128@2x.png");
        Arc::new(gpui::Image::from_bytes(gpui::ImageFormat::Png, png.to_vec()))
    });
    gpui::img(icon.clone()).size(px(88.))
}
