//! The active window's panes as tiles, and the window's `Render`.

use super::*;

/// What every tile of one frame shares.
pub(super) struct Tiling {
    pub(super) sid: String,
    pub(super) active: bool,
    pub(super) target: Option<String>,
    pub(super) several: bool,
    pub(super) stats: Rc<RefCell<Stats>>,
}

impl Workspace {
    /// The active window and the blocks it shows: tiles in leaf order, then floats and drawers.
    pub(super) fn shown(&self) -> Option<(leon_core::Window, Panes)> {
        let w = self.active_window()?;
        let mut ids: Panes = match &w.zoomed {
            Some(z) => vec![z.clone()],
            None => w.layout.as_ref().map(|t| t.leaves().into_iter().map(String::from).collect()).unwrap_or_default(),
        };
        ids.extend(self.overlay_ids(&w));
        let m = self.app.model();
        let s = m.session(self.session_id.as_deref()?)?;
        if w.layout.is_none() && !s.overlays.contains_key(&w.id) {
            return None;
        }
        ids.retain(|id| s.blocks.iter().any(|b| b.id == *id));
        Some((w, ids))
    }

    /// The active window's panes, placed by `panes::tiles` in the pane area (the window less the
    /// sidebar and the titlebar row, so the size is known now, not a frame late) less its
    /// drawers, then the drawers and floats over them (`render_overlays`).
    pub(super) fn render_panes(
        &self,
        w: &leon_core::Window,
        terms: Panes,
        window: &Window,
        stats: Rc<RefCell<Stats>>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let t = &self.theme;
        let size = self.pane_area(window);
        let view = window.viewport_size();
        let origin = (f32::from(view.width) - size.0, f32::from(view.height) - size.1);
        let placed = self.placed(w, size);
        let tiles = placed.tiles;
        let mut l = w
            .layout
            .as_ref()
            .map(|tree| panes::tiles(tree, w.zoomed.as_deref(), (tiles.w, tiles.h)))
            .unwrap_or_default();
        let shift = |r: &mut panes::Rect| (r.x, r.y) = (r.x + tiles.x, r.y + tiles.y);
        l.panes.iter_mut().for_each(|p| shift(&mut p.rect));
        for d in &mut l.dividers {
            shift(&mut d.rect);
            shift(&mut d.split);
        }
        let tiling = Tiling {
            sid: self.session_id.clone().unwrap_or_default(),
            active: window.is_window_active(),
            target: target(w).map(str::to_string),
            several: l.panes.len() + placed.docks.len() + placed.floats.len() > 1,
            stats,
        };
        let mut out =
            div().debug_selector(|| sel("panes-area".into())).flex_1().relative().overflow_hidden().bg(rgb(t.chrome));
        self.app.watch(terms.clone()); // only what is on screen is pushed
        for p in &l.panes {
            if terms.contains(&p.block_id) {
                out = out.child(self.pane_tile("pane", &p.block_id, p.rect, None, &tiling, cx));
            }
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
        self.render_overlays(out, placed, &terms, &tiling, (size, origin), cx)
    }

    /// One pane's tile at `rect`: its header (always on a float or drawer, which `grab` moves when
    /// given), its terminal, the find bar and jump labels, and the focus mark. All but the find bar
    /// (a control the user opened) are in the pane's own theme, faded without the focus. `kind`
    /// names it in tests (`pane-`, `float-`, `dock-`).
    pub(super) fn pane_tile(
        &self,
        kind: &str,
        id: &str,
        rect: panes::Rect,
        grab: Option<crate::floats::Grab>,
        tiling: &Tiling,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let t = &self.pane_theme(id);
        let focused = tiling.target.as_deref() == Some(id);
        let id = id.to_string();
        let view = self.drawn(&id);
        let marks = self.pane_marks(&id, &view);
        let canvas = terminal_canvas(
            self.app.clone(),
            (tiling.sid.clone(), id.clone()),
            (crate::fade::frame(view.frame, t.chrome, self.pane_fade(&id)), marks),
            tiling.active && focused,
            (self.font_family.clone(), self.font_size, *t),
            (tiling.stats.clone(), self.term.painted.clone()),
        );
        let headers = self.settings.pane_headers || kind != "pane";
        let finding = self.term.find.as_ref().is_some_and(|f| f.block == id);
        let on_link = self.term.hovered.as_ref().is_some_and(|(b, _)| *b == id);
        // built only when shown: building an element notes it as drawn (`sel`)
        let header = headers.then(|| match grab {
            Some(g) => div()
                .on_mouse_down(MouseButton::Left, self.grab_listener(&id, g, cx))
                .child(self.render_header(&id, focused, rect.w, cx))
                .into_any_element(),
            None => self.render_header(&id, focused, rect.w, cx).into_any_element(),
        });
        let name = format!("{kind}-{id}");
        div()
            .id(SharedString::from(name.clone()))
            .debug_selector(|| sel(name))
            .group(SharedString::from(format!("pane-{id}")))
            .absolute()
            .left(px(rect.x))
            .top(px(rect.y))
            .w(px(rect.w))
            .h(px(rect.h))
            .flex()
            .flex_col()
            .rounded(px(6.))
            .overflow_hidden()
            .bg(rgb(t.bg))
            .children(header)
            // with headers off the find bar floats over the pane, so its size stays
            .when(!headers && finding, |d| {
                d.child(
                    div()
                        .absolute()
                        .top_0()
                        .left_0()
                        .right_0()
                        .bg(rgb(t.bg))
                        .child(self.render_header(&id, focused, rect.w, cx)),
                )
            })
            .child(self.terminal_area(&id, on_link, cx).child(canvas))
            .children(self.render_jump(&id, cx))
            // the focus mark: the tile's left edge, as the mark's rail holds its pane
            .when(tiling.several && focused, |d| {
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
            })
    }

    /// A pane's terminal area: the mouse selects, reports to the program, scrolls, and opens links.
    fn terminal_area(&self, id: &str, on_link: bool, cx: &mut Context<Self>) -> gpui::Div {
        let mut d = div()
            .debug_selector(|| sel(format!("terminal-{id}")))
            .flex_1()
            .p(px(panes::PADDING))
            .when(on_link, |d| d.cursor(CursorStyle::PointingHand));
        for b in [MouseButton::Left, MouseButton::Middle, MouseButton::Right] {
            let (down, up) = (id.to_string(), id.to_string());
            d = d
                .on_mouse_down(
                    b,
                    cx.listener(move |ws, ev: &gpui::MouseDownEvent, w, cx| ws.mouse_down(&down, ev, w, cx)),
                )
                .on_mouse_up(b, cx.listener(move |ws, ev: &MouseUpEvent, _, cx| ws.mouse_up(&up, ev, cx)));
        }
        // a drag released outside the pane still ends its selection or report there
        let out = id.to_string();
        d = d.on_mouse_up_out(
            MouseButton::Left,
            cx.listener(move |ws, ev: &MouseUpEvent, _, cx| ws.mouse_up(&out, ev, cx)),
        );
        let (moved, wheel) = (id.to_string(), id.to_string());
        d.on_mouse_move(cx.listener(move |ws, ev: &MouseMoveEvent, _, cx| ws.mouse_move(&moved, ev, cx)))
            .on_scroll_wheel(cx.listener(move |ws, ev: &gpui::ScrollWheelEvent, _, cx| ws.wheel(&wheel, ev, cx)))
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
    /// has seen its done and error: clear them (docs/agents.md, "Lifetime"). Not while they stay on it, so a
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
        if std::mem::take(&mut self.refit) {
            self.set_font_size(self.font_size, window, cx); // a new font from init.lua
        }
        self.note_slides();
        DRAWN.with(|d| d.borrow_mut().clear());
        let shown = self.shown().map(|(_, ids)| ids).unwrap_or_default();
        self.term.painted.borrow_mut().retain(|id, _| shown.contains(id)); // hit tests see only these
        self.note_fades(&shown);
        if self.term.find.as_ref().is_some_and(|f| !shown.contains(&f.block)) {
            self.term.find = None; // its pane left the screen
        }
        for id in &shown {
            self.sync_pane(id);
        }
        if self.find_due() {
            self.run_find();
        }
        let stats = self.stats.clone();
        if stats.borrow().enabled {
            stats.borrow_mut().render_start = Some(Instant::now());
        }
        let rows = self.rows();
        self.settle(&rows);
        self.note_recent();
        let current = rows.iter().find(|r| Some(&r.id) == self.session_id.as_ref());
        let body = match self.shown() {
            _ if self.stopped => self.render_stopped(cx).into_any_element(),
            Some((w, terms)) => self.render_panes(&w, terms, window, stats, cx).into_any_element(),
            None => self.render_empty(cx).into_any_element(),
        };
        let places = self.places();
        let column = div()
            .flex_1()
            .flex()
            .flex_col()
            .min_w(px(0.))
            .when(self.titlebar_shown() && !self.stopped, |d| d.child(self.render_titlebar(current, cx)))
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
            .on_action(cx.listener(Self::about))
            .child(self.drag_listener(cx))
            .on_action(cx.listener(Self::new_session))
            .on_action(cx.listener(Self::next_tab))
            .on_action(cx.listener(Self::previous_tab))
            .on_action(cx.listener(Self::paste))
            .on_action(cx.listener(Self::toggle_sidebar))
            .on_key_down(cx.listener(Self::key_down))
            .on_modifiers_changed(cx.listener(Self::modifiers_changed))
            .on_action(cx.listener(|ws, _: &Copy, w, cx| ws.run_action("pane.copy", None, "menu", w, cx)))
            .on_action(cx.listener(|ws, _: &SelectAll, w, cx| ws.run_action("pane.select_all", None, "menu", w, cx)))
            .on_action(cx.listener(|ws, _: &Find, w, cx| ws.run_action("client.find.open", None, "menu", w, cx)))
            .on_action(cx.listener(|ws, _: &OpenSettings, w, cx| ws.run_action("client.settings", None, "menu", w, cx)))
            .size_full()
            .flex()
            .bg(rgb(self.theme.chrome))
            .font_family(".SystemUIFont")
            .text_size(px(UI_TEXT))
            .text_color(rgb(self.theme.text))
            // a stopped server's sessions are gone: nothing of them stays drawn
            .children(places.sidebar.filter(|_| !self.stopped).map(|list| self.render_sidebar(&rows, list, cx)))
            .child(column)
            .children(self.render_stale(window))
            // last, so it paints over the panes: GPUI paints children in order
            .children(self.render_overlay(cx).filter(|_| !self.stopped))
    }
}

pub(super) use crate::switcher::program_name;
