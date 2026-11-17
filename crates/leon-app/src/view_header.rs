//! A pane's header (program, title or cwd, status, caption, and split/zoom/close on the focused or
//! hovered pane), and the empty state shown when there is no session.

use super::*;

/// The widest a header's title gets, and what the header's other parts take from a pane's width
/// (the padding, the icon, a status mark and the hidden controls, which keep their room).
pub(super) const TITLE_MAX: f32 = 320.;
pub(super) const HEADER_PARTS: f32 = 170.;

impl Workspace {
    /// What pane `id`'s host badge takes from its header.
    pub(super) fn badge_width(&self, id: &str) -> f32 {
        // shortcut: the host badge's width from its length, not measured; fine for short labels
        self.pane_host(id).map_or(0., |h| h.chars().count() as f32 * 7. + 16.)
    }

    /// Drawn from `input` (view_sections.rs), whose title is already cut to the pane's width.
    pub(super) fn render_header(&self, input: &HeaderInput, cx: &mut Context<Self>) -> impl IntoElement {
        let id = input.id.as_str();
        let t = &input.theme; // faded with its pane, but for the status colours
        let (shown, caption, focused) = (input.state, input.caption.clone(), input.focused);
        let frame = self.pulse();
        let (title, tip) = input.title.clone();
        let theme = input.window_theme;
        let overlay = input.overlay;
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
            // the pane hovered is an input: a hover group outside a cached view never matches
            .when(!focused && !input.hovered, |d| d.invisible())
            // a float or drawer goes back into the tiles rather than splitting or zooming
            .when(!overlay, |d| {
                d.child(control("split-right", Icon::SplitRight, "pane.split.right", cx))
                    .child(control("split-down", Icon::SplitDown, "pane.split.down", cx))
                    .child(control("zoom", Icon::Zoom, "pane.zoom", cx))
            })
            .when(overlay, |d| d.child(control("unfloat", Icon::Unfloat, "pane.unfloat", cx)))
            .child(control("close", Icon::Close, "pane.close", cx));
        div()
            .debug_selector(|| sel(format!("header-{id}")))
            .flex()
            .flex_none()
            .items_center()
            .gap(px(6.))
            // laid out as a cached view's root, like the title bar: the caption's flex_1 needs a
            // width to push the controls to the right end
            .w_full()
            .h(px(panes::HEADER))
            .pl(px(10.))
            .pr(px(4.))
            .text_size(px(SECONDARY_TEXT))
            .child(program_mark(input.program, format!("header-program-{id}"), t.muted))
            .child(
                div()
                    .id(SharedString::from(format!("header-title-{id}")))
                    .debug_selector(|| sel(format!("header-title-{id}")))
                    .when_some(tip, |d, tip| {
                        let tip = SharedString::from(tip);
                        d.tooltip(move |_, cx| cx.new(|_| super::bars::Tip(tip.clone(), theme)).into())
                    })
                    .flex_shrink()
                    .max_w(px(TITLE_MAX))
                    .overflow_hidden()
                    .min_w(px(0.))
                    .line_clamp(1)
                    .text_ellipsis()
                    .text_color(rgb(if focused { t.title } else { t.text }))
                    .child(title),
            )
            .children(input.host.clone().map(|host| {
                div()
                    .debug_selector(|| sel(format!("header-host-{id}")))
                    .flex_none()
                    .px(px(5.))
                    .rounded(px(4.))
                    .bg(rgb(t.hover))
                    .text_color(rgb(t.muted))
                    .child(host)
            }))
            .children(shown.map(|i| status_mark(i, frame, t).debug_selector(|| sel(format!("header-status-{id}")))))
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

    /// No session: the name, the tagline, and the things to know, each one a click away. Sized for
    /// a big window: one column, keys in a fixed-width lane so the descriptions line up.
    pub(super) fn render_empty(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = &self.theme;
        let mut hints = div().flex().flex_col().gap(px(6.)).mt(px(36.)).w_full();
        for (i, (keys, what, action)) in self.empty_items().into_iter().enumerate() {
            hints = hints.child(
                div()
                    .id(SharedString::from(format!("hint-{i}")))
                    .debug_selector(|| sel(format!("hint-{i}")))
                    .flex()
                    .items_center()
                    .gap(px(18.))
                    .h(px(46.))
                    .px(px(14.))
                    .rounded(px(8.))
                    .when(action.is_some(), |d| d.cursor_pointer().hover(|d| d.bg(rgb(t.hover))))
                    .child(
                        div().flex_none().w(px(150.)).flex().justify_end().child(
                            div()
                                .px(px(8.))
                                .py(px(2.))
                                .rounded(px(5.))
                                .bg(rgb(t.hover))
                                .text_color(rgb(t.accent))
                                .font_family(self.font_family.clone())
                                .text_size(px(UI_TEXT + 1.))
                                .child(keys),
                        ),
                    )
                    .child(div().flex_1().text_size(px(UI_TEXT + 2.)).text_color(rgb(t.text)).child(what))
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
        let content = div()
            .debug_selector(|| sel("empty-content".into()))
            .w(px(520.))
            .flex()
            .flex_col()
            .items_center()
            .child(mark())
            .child(
                div()
                    .debug_selector(|| sel("empty-title".into()))
                    .mt(px(20.))
                    .text_size(px(44.))
                    .text_color(rgb(t.title))
                    .child("leon"),
            )
            .child(
                div().mt(px(4.)).text_size(px(UI_TEXT + 4.)).text_color(rgb(t.muted)).child("Keep your work in view."),
            )
            .child(hints);
        div()
            .debug_selector(|| sel("empty-state".into()))
            .flex_1()
            .bg(self.chrome_fill())
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            // a little above the middle, where the eye expects the centre to be
            .pb(px(48.))
            .child(content)
    }
}

impl Workspace {
    /// This Mac's server stopped from outside (`leon server stop`): say so, and offer to start it
    /// again (its saved sessions come back) or to quit.
    pub(super) fn render_stopped(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = &self.theme;
        let button = |name: &'static str, label: &'static str, primary: bool| {
            div()
                .id(name)
                .debug_selector(move || sel(name.into()))
                .px(px(16.))
                .py(px(7.))
                .rounded(px(6.))
                .cursor_pointer()
                .bg(rgb(if primary { t.accent } else { t.hover }))
                .text_color(rgb(if primary { t.chrome } else { t.title }))
                .child(label)
        };
        div()
            .debug_selector(|| sel("server-stopped".into()))
            .flex_1()
            .bg(self.chrome_fill())
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .pb(px(48.))
            .child(mark())
            .child(
                div().mt(px(20.)).text_size(px(UI_TEXT + 9.)).text_color(rgb(t.title)).child("The leon server stopped"),
            )
            .child(
                div()
                    .mt(px(6.))
                    .text_size(px(UI_TEXT + 1.))
                    .text_color(rgb(t.muted))
                    .child("Start it again to bring back the sessions it saved, or quit."),
            )
            .child(
                div()
                    .mt(px(28.))
                    .flex()
                    .gap(px(10.))
                    .child(
                        button("server-stopped-start", "Start It Again", true)
                            .on_mouse_down(MouseButton::Left, cx.listener(|ws, _, _, cx| ws.start_again(cx))),
                    )
                    .child(
                        button("server-stopped-quit", "Quit", false)
                            .on_mouse_down(MouseButton::Left, cx.listener(|_, _, _, cx| cx.quit())),
                    ),
            )
    }
}

/// leon's icon (the lion), as the Dock shows it.
fn mark() -> impl IntoElement {
    static ICON: std::sync::OnceLock<Arc<gpui::Image>> = std::sync::OnceLock::new();
    let icon = ICON.get_or_init(|| {
        let png = include_bytes!("../../../brand/icon/Leon.iconset/icon_128x128@2x.png");
        Arc::new(gpui::Image::from_bytes(gpui::ImageFormat::Png, png.to_vec()))
    });
    gpui::img(icon.clone()).debug_selector(|| sel("empty-mark".into())).size(px(132.))
}
