//! What floats over the panes: the command palette, and the hint for a pending key sequence.

use super::*;

/// Palette rows shown at once; the query narrows the rest.
const PALETTE_ROWS: usize = 12;

impl Workspace {
    pub(super) fn render_overlay(&self, cx: &mut Context<Self>) -> Option<gpui::AnyElement> {
        if self.palette_open() {
            return Some(self.render_palette(cx).into_any_element());
        }
        self.pending_hint().map(|hint| render_hint(hint).into_any_element())
    }

    fn render_palette(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = &DARK;
        let query = self.palette_query();
        let selected = self.palette_selected();
        let matches = self.palette_matches();
        // keep the selected row in view
        let first = selected.saturating_sub(PALETTE_ROWS - 1);
        let mut list = div().flex().flex_col().py(px(4.));
        for (i, item) in matches.iter().enumerate().skip(first).take(PALETTE_ROWS) {
            let name = item.name.clone();
            list = list.child(
                div()
                    .id(SharedString::from(format!("palette-{i}")))
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(px(12.))
                    .h(px(28.))
                    .px(px(12.))
                    .mx(px(4.))
                    .rounded(px(5.))
                    .cursor_pointer()
                    .when(i == selected, |d| d.bg(rgb(t.selected)).text_color(rgb(t.title)))
                    .when(i != selected, |d| d.text_color(rgb(t.text)))
                    .child(
                        div()
                            .flex_1()
                            .overflow_hidden()
                            .min_w(px(0.))
                            .line_clamp(1)
                            .text_ellipsis()
                            .child(item.title.clone()),
                    )
                    .children(
                        item.shortcut
                            .clone()
                            .map(|k| div().text_size(px(SECONDARY_TEXT)).text_color(rgb(t.muted)).child(k)),
                    )
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |ws, _, window, cx| {
                            ws.palette = None;
                            ws.run_action(&name, None, "palette", window, cx);
                            cx.stop_propagation();
                        }),
                    ),
            );
        }
        if matches.is_empty() {
            list = list.child(div().px(px(16.)).py(px(6.)).text_color(rgb(t.muted)).child("No matching actions"));
        }
        let prompt = if query.is_empty() {
            div().text_color(rgb(t.muted)).child("Type an action…")
        } else {
            div().text_color(rgb(t.text)).child(query)
        };
        div().absolute().top(px(TITLEBAR + 24.)).left_0().right_0().flex().justify_center().child(
            div()
                .id("palette")
                .w(px(520.))
                .flex()
                .flex_col()
                .bg(rgb(t.chrome))
                .border_1()
                .border_color(rgb(t.border))
                .rounded(px(10.))
                .shadow_lg()
                .occlude()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .h(px(40.))
                        .px(px(14.))
                        .border_b_1()
                        .border_color(rgb(t.border))
                        .child(div().text_color(rgb(t.accent)).child(Icon::Search.at(TOOLBAR_ICON, t.accent)))
                        .child(prompt),
                )
                .child(list),
        )
    }
}

/// A pending sequence (`ctrl+b`) and what may follow it, bottom right.
fn render_hint((prefix, next): (String, Vec<(String, String)>)) -> impl IntoElement {
    let t = &DARK;
    let mut rows = div().flex().flex_col().gap(px(2.));
    for (key, what) in next {
        rows = rows.child(
            div()
                .flex()
                .gap(px(10.))
                .text_size(px(SECONDARY_TEXT))
                .child(div().min_w(px(48.)).text_color(rgb(t.accent)).child(key))
                .child(div().text_color(rgb(t.text)).child(what)),
        );
    }
    div()
        .absolute()
        .bottom(px(16.))
        .right(px(16.))
        .p(px(10.))
        .flex()
        .flex_col()
        .gap(px(6.))
        .bg(rgb(t.chrome))
        .border_1()
        .border_color(rgb(t.border))
        .rounded(px(8.))
        .shadow_lg()
        .child(
            div().text_size(px(SECONDARY_TEXT)).text_color(rgb(t.muted)).child(format!("{prefix} ›   esc to cancel")),
        )
        .child(rows)
}
