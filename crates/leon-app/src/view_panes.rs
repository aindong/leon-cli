//! The active window's panes as tiles, and the window's `Render`.

use super::*;

/// What a drawn frame showed, for the bookkeeping after it (`drew_frame`): the model can move
/// between the frame and then.
pub(super) struct Shown {
    /// (session, block) input went to, on `app`'s machine.
    target: Option<(String, String)>,
    active: bool,
    app: Rc<Remote>,
}

/// What every tile of one frame shares.
pub(super) struct Tiling {
    pub(super) target: Option<String>,
    pub(super) several: bool,
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

    /// Each pane of window `w` with its rect's size and kind (`pane`, `dock`, `float`), as
    /// `render_panes` lays them out (a slide moves a drawer, never resizes it).
    pub(super) fn pane_rects(
        &self,
        w: &leon_core::Window,
        window: &Window,
    ) -> Vec<(String, panes::Rect, &'static str)> {
        let placed = self.placed(w, self.pane_area(window));
        let t = placed.tiles;
        let l = w.layout.as_ref().map(|tree| panes::tiles(tree, w.zoomed.as_deref(), (t.w, t.h))).unwrap_or_default();
        let tiles = l.panes.into_iter().map(|p| (p.block_id, p.rect, "pane"));
        let docks = placed.docks.into_iter().map(|d| (d.block_id, d.rect, "dock"));
        tiles.chain(docks).chain(placed.floats.into_iter().map(|f| (f.block_id, f.rect, "float"))).collect()
    }

    /// The active window's panes, placed by `panes::tiles` in the pane area (the window less the
    /// sidebar and the titlebar row, so the size is known now, not a frame late) less its
    /// drawers, then the drawers and floats over them (`render_overlays`).
    pub(super) fn render_panes(
        &self,
        w: &leon_core::Window,
        terms: Panes,
        window: &Window,
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
            target: target(w).map(str::to_string),
            several: l.panes.len() + placed.docks.len() + placed.floats.len() > 1,
        };
        let mut out =
            div().debug_selector(|| sel("panes-area".into())).flex_1().relative().overflow_hidden().bg(rgb(t.chrome));
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
        let canvas = self.grid_element(&id);
        let headers = self.settings.pane_headers || kind != "pane";
        let finding = self.term.find.as_ref().is_some_and(|f| f.block == id);
        let on_link = self.term.hovered.as_ref().is_some_and(|(b, _)| *b == id);
        // built only when shown: building an element notes it as drawn (`sel`)
        let header = headers.then(|| match grab {
            Some(g) => div()
                .on_mouse_down(MouseButton::Left, self.grab_listener(&id, g, cx))
                .child(self.header_section(&id, rect.w, cx))
                .into_any_element(),
            None => self.header_section(&id, rect.w, cx),
        });
        let name = format!("{kind}-{id}");
        div()
            .id(SharedString::from(name.clone()))
            .debug_selector(|| sel(name))
            .group(SharedString::from(format!("pane-{id}")))
            .on_hover({
                let id = id.clone();
                cx.listener(move |ws, on: &bool, _, cx| {
                    if *on {
                        ws.hovered_pane = Some(id.clone());
                    } else if ws.hovered_pane.as_ref() == Some(&id) {
                        ws.hovered_pane = None;
                    }
                    cx.notify();
                })
            })
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
                        .child(self.header_section(&id, rect.w, cx)),
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
    /// finish in the pane they are looking at still shows until they come back. After a frame
    /// (`drew_frame`), for what that frame `showed`: a hidden window draws nothing, so a pane it
    /// moves to unseen has seen nothing. The poll tells the server.
    pub(super) fn note_focus(&mut self, showed: Shown) {
        let now = showed.target.as_ref().map(|(_, id)| (id.clone(), showed.active));
        let arrived = match (&self.seen_focus, &now) {
            (_, None) => false,
            (Some((was, was_active)), Some((id, active))) => was != id || (*active && !was_active),
            (None, Some(_)) => true,
        };
        self.seen_focus = now;
        if let Some((sid, id)) = showed.target.filter(|_| arrived) {
            let event = json!({ "session_id": sid, "block_id": id, "event": { "type": "focus" } });
            self.focus_event = Some((showed.app, event));
            (self.wake)();
        }
    }

    /// The root's last element: its paint ends the frame for `LEON_FRAME_STATS`, whether or not a
    /// pane's grid painted in it.
    fn frame_mark(&self) -> impl IntoElement {
        let stats = self.stats.clone();
        canvas(
            |_, _, _| (),
            move |_, _, window, _| {
                if stats.borrow().enabled && stats.borrow_mut().frame_end() {
                    window.on_next_frame(move |_, _| stats.borrow_mut().presented());
                }
            },
        )
        .absolute()
        .w(px(0.))
        .h(px(0.))
    }

    /// Before a frame: what render draws from, brought up to date with the model and the window
    /// (ADR R5). It runs after every notify of the workspace, before the frame that notify asks
    /// for (the observer in `with_hosts`), so render, which may run more often (a hover, a
    /// resize), only draws. It takes no `cx`: it can't notify, which would run it again.
    pub(super) fn prepare(&mut self) {
        self.prepares += 1;
        self.note_slides();
        self.settle_overview();
        self.drop_unshown_thumbs();
        let shown = self.shown().map(|(_, ids)| ids).unwrap_or_default();
        self.term.painted.borrow_mut().retain(|id, _| shown.contains(id)); // hit tests see only these
        if self.term.find.as_ref().is_some_and(|f| !shown.contains(&f.block)) {
            self.term.find = None; // its pane left the screen
        }
        for id in &shown {
            self.sync_pane(id);
        }
        // a window drawing nothing (hidden, covered) asks the server again once it draws (`next_poll`)
        if self.find_due() && self.undrawn < 2 {
            self.run_find();
        }
        let rows = self.rows();
        self.settle(&rows);
        self.note_recent();
        // after `settle`, which may have moved to another session: the fades are aimed at the
        // very panes the frame draws
        let on_screen = self.shown().map(|(_, ids)| ids).unwrap_or_default();
        self.note_fades(&on_screen);
        // only what is drawn is pushed: the panes on screen, and every thumbnail's (view_overview.rs);
        // the panes stay watched under the overview, so they are current when it closes, even when
        // a filter hid their card
        if !self.stopped {
            self.watch_screens(&on_screen);
        }
    }
}

impl Render for Workspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        drawing(cx);
        self.renders += 1;
        // after the frame: undrawn redraws back to none, the pane it showed noted as seen, and the
        // poll woken if that is news or something timed is due sooner (ADR R5, frame bookkeeping)
        let showed = Shown {
            target: self.target().map(|(sid, id, _)| (sid, id)),
            active: window.is_window_active(),
            app: self.app.clone(),
        };
        cx.defer_in(window, move |ws, _, cx| ws.drew_frame(showed, cx));
        let stats = self.stats.clone();
        if stats.borrow().enabled {
            stats.borrow_mut().render_start = Some(Instant::now());
        }
        let body_shown = self.shown();
        let preview = self.render_preview(window, cx).filter(|_| !self.stopped);
        // the overview takes the sidebar's and the panes' place, in the same root as they are, so
        // the menu's actions and the window's listeners still reach the workspace
        let main: Vec<gpui::AnyElement> = if self.overview_shown() && !self.stopped {
            vec![self.render_overview(window, cx)]
        } else {
            let body = match body_shown {
                _ if self.stopped => self.render_stopped(cx).into_any_element(),
                Some((w, terms)) => self.render_panes(&w, terms, window, cx).into_any_element(),
                None => self.render_empty(cx).into_any_element(),
            };
            let column = div()
                .flex_1()
                .flex()
                .flex_col()
                .min_w(px(0.))
                .when(self.titlebar_shown() && !self.stopped, |d| d.child(self.titlebar_section(cx)))
                .child(body);
            // a stopped server's sessions are gone: nothing of them stays drawn
            let sidebar = self.places().sidebar.filter(|_| !self.stopped).map(|_| self.sidebar_section(cx));
            sidebar.into_iter().chain([column.into_any_element()]).collect()
        };
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
            .children(main)
            .children(self.render_stale(window))
            .children(self.render_update(cx).filter(|_| !self.stopped))
            .children(preview)
            // last, so it paints over the panes: GPUI paints children in order
            .children(self.render_overlay(cx).filter(|_| !self.stopped))
            .child(self.frame_mark())
    }
}

pub(super) use crate::switcher::program_name;
