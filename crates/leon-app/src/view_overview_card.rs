//! An overview card (a tab's thumbnail under its header) and the frame stats' mark.

use super::*;

impl Workspace {
    /// One card: its header (status, title, the state in words), then its thumbnail; a click goes
    /// to its tab.
    pub(super) fn card(
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
    pub(super) fn stats_mark(&self) -> impl IntoElement {
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
