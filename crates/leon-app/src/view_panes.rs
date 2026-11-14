//! The active window's panes as tiles, and the window's `Render`.

use super::*;

impl Workspace {
    /// The active window's panes, placed by `panes::tiles` in the pane area: the window less the
    /// sidebar and the titlebar row (so the size is known now, not a frame late).
    pub(super) fn render_panes(
        &self,
        w: &leon_core::Window,
        terms: Panes,
        window: &Window,
        stats: Rc<RefCell<Stats>>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let t = &DARK;
        let size = self.pane_area(window);
        let active = window.is_window_active();
        let font_size = self.font_size;
        let l = w.layout.as_ref().map(|tree| panes::tiles(tree, w.zoomed.as_deref(), size)).unwrap_or_default();
        let target = target(w).map(str::to_string);
        let several = l.panes.len() > 1;
        let headers = self.settings.pane_headers;
        let mut out = div().flex_1().relative().bg(rgb(t.chrome));
        self.app.watch(terms.clone()); // only what is on screen is pushed
        let sid = self.session_id.clone().unwrap_or_default();
        for p in &l.panes {
            if !terms.contains(&p.block_id) {
                continue;
            }
            let focused = target.as_deref() == Some(p.block_id.as_str());
            let id = p.block_id.clone();
            let canvas = terminal_canvas(
                self.app.clone(),
                (sid.clone(), id.clone()),
                active && focused,
                font_size,
                stats.clone(),
            );
            out = out.child(
                div()
                    .id(SharedString::from(format!("pane-{id}")))
                    .debug_selector(|| sel(format!("pane-{id}")))
                    .group(SharedString::from(format!("pane-{id}")))
                    .absolute()
                    .left(px(p.rect.x))
                    .top(px(p.rect.y))
                    .w(px(p.rect.w))
                    .h(px(p.rect.h))
                    .flex()
                    .flex_col()
                    .rounded(px(6.))
                    .overflow_hidden()
                    .bg(rgb(t.bg))
                    .when(headers, |d| d.child(self.render_header(&id, focused, cx)))
                    .child(
                        div()
                            .debug_selector(|| sel(format!("terminal-{id}")))
                            .flex_1()
                            .p(px(panes::PADDING))
                            .child(canvas),
                    )
                    // the focus mark: the tile's left edge, as the mark's rail holds its pane
                    .when(several && focused, |d| {
                        d.child(
                            div()
                                .debug_selector(|| sel(format!("focus-edge-{id}")))
                                .absolute()
                                .left_0()
                                .top_0()
                                .bottom_0()
                                .w(px(2.))
                                .bg(rgb(t.accent)),
                        )
                    })
                    .on_mouse_down(MouseButton::Left, {
                        let id = id.clone();
                        cx.listener(move |ws, _, _, cx| ws.focus_block(&id, cx))
                    }),
            );
        }
        for (i, d) in l.dividers.iter().enumerate() {
            let r = d.rect;
            let across = d.direction == Direction::Horizontal;
            let divider = d.clone();
            // the channel between two tiles is the handle
            out = out.child(
                div()
                    .id(SharedString::from(format!("divider-{i}")))
                    .debug_selector(|| sel(format!("divider-{i}")))
                    .absolute()
                    .left(px(r.x))
                    .top(px(r.y))
                    .w(px(r.w))
                    .h(px(r.h))
                    .occlude()
                    .cursor(if across { CursorStyle::ResizeLeftRight } else { CursorStyle::ResizeUpDown })
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |ws, ev: &gpui::MouseDownEvent, _, cx| {
                            ws.start_drag(divider.clone(), ev.position, cx)
                        }),
                    ),
            );
        }
        out
    }

    /// The pane area's size (w, h): the window less the sidebar and the titlebar row.
    pub(super) fn pane_area(&self, window: &Window) -> (f32, f32) {
        let view = window.viewport_size();
        let sidebar = if self.places().sidebar.is_some() { self.settings.sidebar_width } else { 0. };
        let top = if self.titlebar_shown() { TITLEBAR } else { 0. };
        ((f32::from(view.width) - sidebar).max(0.), (f32::from(view.height) - top).max(0.))
    }

    /// The titlebar row shows when the sidebar can't hold the traffic lights, or for top tabs.
    pub(super) fn titlebar_shown(&self) -> bool {
        let p = self.places();
        p.sidebar.is_none() || p.top_tabs
    }

    /// The user arriving at a pane (another pane, tab or session selected, or the window made active)
    /// has seen its done and error: clear them (spec §5 lifetime). Not while they stay on it, so a
    /// finish in the pane they are looking at still shows until they come back.
    fn note_focus(&mut self, window: &Window) {
        let now = self.target().map(|(_, id, _)| (id, window.is_window_active()));
        let arrived = match (&self.seen_focus, &now) {
            (_, None) => false,
            (Some((was, was_active)), Some((id, active))) => was != id || (*active && !was_active),
            (None, Some(_)) => true,
        };
        self.seen_focus = now;
        if let Some((sid, id, _)) = self.target().filter(|_| arrived) {
            let event = json!({ "session_id": sid, "block_id": id, "event": { "type": "focus" } });
            self.app.send_then("block.event", event, drop); // a pane already gone is fine
        }
    }
}

impl Render for Workspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.note_focus(window);
        DRAWN.with(|d| d.borrow_mut().clear());
        let stats = self.stats.clone();
        if stats.borrow().enabled {
            stats.borrow_mut().render_start = Some(Instant::now());
        }
        let rows = self.rows();
        self.settle(&rows);
        let current = rows.iter().find(|r| Some(&r.id) == self.session_id.as_ref());
        let body = match self.shown() {
            Some((w, terms)) => self.render_panes(&w, terms, window, stats, cx).into_any_element(),
            None => self.render_empty(cx).into_any_element(),
        };
        let places = self.places();
        let column = div()
            .flex_1()
            .flex()
            .flex_col()
            .min_w(px(0.))
            .when(self.titlebar_shown(), |d| d.child(self.render_titlebar(current, cx)))
            .child(body);
        div()
            .key_context("Workspace")
            .track_focus(&self.focus)
            .on_action(cx.listener(Self::new_tab))
            .on_action(cx.listener(Self::close_pane))
            .on_action(cx.listener(Self::split_right))
            .on_action(cx.listener(Self::split_down))
            .on_action(cx.listener(Self::focus_left))
            .on_action(cx.listener(Self::focus_right))
            .on_action(cx.listener(Self::focus_up))
            .on_action(cx.listener(Self::focus_down))
            .on_action(cx.listener(Self::toggle_zoom))
            .on_action(cx.listener(Self::quit_and_end_all))
            .child(self.drag_listener(cx))
            .on_action(cx.listener(Self::new_session))
            .on_action(cx.listener(Self::next_tab))
            .on_action(cx.listener(Self::previous_tab))
            .on_action(cx.listener(Self::paste))
            .on_action(cx.listener(Self::toggle_sidebar))
            .on_key_down(cx.listener(Self::key_down))
            .size_full()
            .flex()
            .bg(rgb(DARK.chrome))
            .font_family(".SystemUIFont")
            .text_size(px(UI_TEXT))
            .text_color(rgb(DARK.text))
            .children(places.sidebar.map(|list| self.render_sidebar(&rows, list, cx)))
            .child(column)
            .children(self.render_stale(window))
            // last, so it paints over the panes: GPUI paints children in order
            .children(self.render_overlay(cx))
    }
}

/// The argv0 a tab shows: `/bin/zsh` → `zsh`, a login shell's `-zsh` → `zsh`.
pub(super) fn program_name(command: &[String]) -> String {
    let argv0 = command.first().map_or("", String::as_str);
    argv0.rsplit('/').next().unwrap_or(argv0).trim_start_matches('-').to_string()
}
