//! The workspace's actions and input: splits, focus, divider drags, tabs, paste and font size.

use super::*;

impl Workspace {
    /// Split the target pane, the new pane on `side` ("before"/"after") and in the pane's cwd; a
    /// zoom ends first so the new pane shows.
    pub(super) fn split(&mut self, direction: Direction, side: &str, cx: &mut Context<Self>) {
        let Some((_, id, zoomed)) = self.target() else { return };
        if zoomed {
            self.call("session.zoom_block", json!({ "block_id": id, "zoom": false }), cx);
        }
        let p = json!({ "anchor_id": id, "direction": direction, "side": side, "layout": self.here(), "focus": true });
        self.call("session.new_split", p, cx);
    }

    /// A layout for a shell in the target pane's working directory (or the default, `$HOME`).
    fn here(&self) -> Value {
        let pwd = |(sid, id, _): (String, String, bool)| {
            let p = json!({ "session_id": sid, "block_id": id, "method": "dev.leon.terminal.pwd" });
            self.app.call("block.method", p).ok()?["path"].as_str().map(std::path::PathBuf::from)
        };
        let cwd = self.target().and_then(pwd).filter(|d| d.is_dir());
        match cwd.as_deref().and_then(|d| d.to_str()) {
            Some(d) => json!({ "block": { "options": { "cwd": d } } }),
            None => json!({ "block": {} }),
        }
    }

    /// Move focus by direction from the pane on screen; a zoom ends first so the result shows.
    pub(super) fn focus_toward(&mut self, toward: &str, cx: &mut Context<Self>) {
        let Some((_, id, zoomed)) = self.target() else { return };
        if zoomed {
            self.call("session.focus_block", json!({ "block_id": id }), cx);
            self.call("session.zoom_block", json!({ "block_id": id, "zoom": false }), cx);
        }
        self.call("session.focus_direction", json!({ "direction": toward }), cx);
    }

    pub(super) fn split_right(&mut self, _: &SplitRight, _: &mut Window, cx: &mut Context<Self>) {
        self.split(Direction::Horizontal, "after", cx);
    }

    pub(super) fn split_down(&mut self, _: &SplitDown, _: &mut Window, cx: &mut Context<Self>) {
        self.split(Direction::Vertical, "after", cx);
    }

    pub(super) fn focus_left(&mut self, _: &FocusLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.focus_toward("left", cx);
    }

    pub(super) fn focus_right(&mut self, _: &FocusRight, _: &mut Window, cx: &mut Context<Self>) {
        self.focus_toward("right", cx);
    }

    pub(super) fn focus_up(&mut self, _: &FocusUp, _: &mut Window, cx: &mut Context<Self>) {
        self.focus_toward("up", cx);
    }

    pub(super) fn focus_down(&mut self, _: &FocusDown, _: &mut Window, cx: &mut Context<Self>) {
        self.focus_toward("down", cx);
    }

    pub(super) fn toggle_zoom(&mut self, _: &ToggleZoom, _: &mut Window, cx: &mut Context<Self>) {
        if let Some((_, id, _)) = self.target() {
            self.call("session.zoom_block", json!({ "block_id": id }), cx);
        }
    }

    /// Close the target pane; its window closes with its last pane, and its session with its last window.
    pub(super) fn close_pane(&mut self, _: &ClosePane, _: &mut Window, cx: &mut Context<Self>) {
        if let Some((_, id, _)) = self.target() {
            self.call("block.close", json!({ "block_id": id }), cx);
        }
    }

    pub(super) fn focus_block(&mut self, id: &str, cx: &mut Context<Self>) {
        self.call("session.focus_block", json!({ "block_id": id }), cx);
    }

    pub(super) fn start_drag(&mut self, divider: Divider, at: gpui::Point<Pixels>, cx: &mut Context<Self>) {
        let from = if divider.direction == Direction::Horizontal { at.x } else { at.y };
        self.drag = Some(Drag { divider, from, last: None });
        cx.notify(); // the next frame installs the window-wide drag listeners
        cx.stop_propagation();
    }

    /// While dragging, the split's ratio follows the mouse: the start ratio plus the distance moved
    /// over the room the split shares out.
    pub(super) fn drag_to(&mut self, ev: &MouseMoveEvent, cx: &mut Context<Self>) {
        let Some(drag) = &self.drag else { return };
        if ev.pressed_button != Some(MouseButton::Left) {
            self.drag = None;
            return;
        }
        let d = &drag.divider;
        let (at, room) = match d.direction {
            Direction::Horizontal => (ev.position.x, d.split.w - DIVIDER),
            Direction::Vertical => (ev.position.y, d.split.h - DIVIDER),
        };
        let moved = f32::from(at - drag.from) as f64;
        // Keep a ratio set elsewhere (say 0.01 from the CLI) where it is until the drag moves it.
        let ratio = (d.ratio + moved / room.max(1.) as f64).clamp(d.ratio.min(0.05), d.ratio.max(0.95));
        if drag.last == Some(ratio) {
            return; // each call is a layout change with events for every subscriber
        }
        let p =
            json!({ "direction": d.direction, "before_block_id": d.before, "after_block_id": d.after, "ratio": ratio });
        if let Some(d) = self.drag.as_mut() {
            d.last = Some(ratio);
        }
        self.call("session.resize_pane", p, cx);
    }

    pub(super) fn end_drag(&mut self) {
        self.drag = None;
    }

    /// While a divider is dragged, follow the mouse anywhere in the window: element listeners only
    /// hear the mouse over their own unblocked area, and the divider itself blocks it.
    pub(super) fn drag_listener(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let ws = cx.entity().downgrade();
        let dragging = self.drag.is_some();
        canvas(
            |_, _, _| {},
            move |_, _, window, _| {
                if !dragging {
                    return;
                }
                let moved = ws.clone();
                window.on_mouse_event(move |ev: &MouseMoveEvent, phase, _, cx| {
                    if phase == gpui::DispatchPhase::Capture {
                        moved.update(cx, |ws, cx| ws.drag_to(ev, cx)).ok();
                    }
                });
                let up = ws.clone();
                window.on_mouse_event(move |_: &MouseUpEvent, phase, _, cx| {
                    if phase == gpui::DispatchPhase::Capture {
                        up.update(cx, |ws, _| ws.end_drag()).ok();
                    }
                });
            },
        )
        .absolute()
        .size_0()
    }

    /// (session id, window id) of the selected session's window `index`, or of its active one.
    pub(super) fn window_ref(&self, index: Option<usize>) -> Option<(String, String)> {
        let sid = self.session_id.clone()?;
        let m = self.app.model();
        let s = m.session(&sid)?;
        let wid = s.windows.get(index.unwrap_or(s.active))?.id.clone();
        Some((sid, wid))
    }

    pub(super) fn focus_window(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some((sid, wid)) = self.window_ref(Some(index)) {
            let _ = self.app.act("session.focus_window", json!({ "session_id": sid, "window_id": wid }));
            cx.notify();
        }
    }

    /// Close a window; if that ended its session, select a neighbouring one.
    pub(super) fn close_window(&mut self, (sid, wid): (String, String), cx: &mut Context<Self>) {
        let _ = self.app.act("session.close_window", json!({ "session_id": sid, "window_id": wid }));
        let rows = self.rows();
        self.settle(&rows);
        cx.notify();
    }

    pub(super) fn step_tab(&mut self, by: isize, cx: &mut Context<Self>) {
        let Some(sid) = self.session_id.clone() else { return };
        let Some((n, active)) = self.app.model().session(&sid).map(|s| (s.windows.len(), s.active)) else {
            return;
        };
        if n > 0 {
            self.focus_window((active as isize + by).rem_euclid(n as isize) as usize, cx);
        }
    }

    pub(super) fn new_tab(&mut self, _: &NewTab, window: &mut Window, cx: &mut Context<Self>) {
        match self.session_id.clone() {
            Some(sid) => drop(self.app.act("session.new_window", json!({ "session_id": sid, "layout": self.here() }))),
            None => self.new_session(&NewSession, window, cx),
        }
        cx.notify();
    }

    pub(super) fn new_session(&mut self, _: &NewSession, _: &mut Window, cx: &mut Context<Self>) {
        if let Ok(r) = self.app.act("session.create", json!({})) {
            self.session_id = r["session_id"].as_str().map(String::from);
        }
        cx.notify();
    }

    pub(super) fn next_tab(&mut self, _: &NextTab, _: &mut Window, cx: &mut Context<Self>) {
        self.step_tab(1, cx);
    }

    pub(super) fn previous_tab(&mut self, _: &PreviousTab, _: &mut Window, cx: &mut Context<Self>) {
        self.step_tab(-1, cx);
    }

    pub(super) fn paste(&mut self, _: &Paste, _: &mut Window, cx: &mut Context<Self>) {
        let (Some((sid, id, _)), Some(text)) = (self.target(), cx.read_from_clipboard().and_then(|c| c.text())) else {
            return;
        };
        if text.len() > MAX_PASTE {
            eprintln!("leon: not pasting {} bytes: at most {MAX_PASTE} go in one paste", text.len());
            return;
        }
        // the server wraps it by the program's mode now; the app's copy of the screen may be older
        let event = json!({ "session_id": sid, "block_id": id, "event": { "type": "paste", "data": text } });
        if let Err(e) = self.app.call("block.event", event) {
            eprintln!("leon: the paste failed: {}", e.message);
        }
    }

    /// Ask once, naming how many sessions end; on yes, stop the server (it saves, then every
    /// program ends) and quit. The app quits anyway when the server goes.
    pub(super) fn quit_and_end_all(&mut self, _: &QuitAndEndAll, window: &mut Window, cx: &mut Context<Self>) {
        let n = self.app.model().sessions.len();
        let what = if n == 1 { "1 session".to_string() } else { format!("{n} sessions") };
        let detail = "Every program in them stops. Quit leon (⌘Q) instead to leave them running.";
        let buttons = [PromptButton::Ok("End All Sessions".into()), PromptButton::Cancel("Cancel".into())];
        let answer = window.prompt(PromptLevel::Warning, &format!("End {what} and quit?"), Some(detail), &buttons, cx);
        let app = self.app.clone();
        cx.spawn(async move |_, cx| {
            if answer.await == Ok(0) {
                if let Err(e) = app.call("server.stop", json!({})) {
                    eprintln!("leon: can't stop the server: {}", e.message);
                    return;
                }
                let _ = cx.update(|cx| cx.quit());
            }
        })
        .detach();
    }

    pub(super) fn toggle_sidebar(&mut self, _: &ToggleSidebar, _: &mut Window, cx: &mut Context<Self>) {
        self.sidebar = !self.sidebar;
        cx.notify();
    }

    /// Set the terminal font size (clamped) and fit every terminal to it at once, hidden tabs too, so
    /// `capture` and agents see the new size before the tab is shown.
    pub(super) fn set_font_size(&mut self, pt: f32, window: &mut Window, cx: &mut Context<Self>) {
        self.font_size = pt.clamp(FONT_SIZES.0, FONT_SIZES.1);
        let (cell_w, line_h) = cell_size(window, self.font_size);
        let area = self.pane_area(window);
        // (session, block, its pane's text area) for every pane of every window, then resize off the lock
        let fits: Vec<(String, String, panes::Rect)> = {
            let m = self.app.model();
            let mut out = Vec::new();
            for s in &m.sessions {
                for w in &s.windows {
                    let Some(tree) = &w.layout else { continue };
                    // panes a zoom hides keep their split size; the zoomed one fills the area
                    let zoomed = w.zoomed.as_deref().map(|z| panes::layout(tree, Some(z), area, DIVIDER).panes);
                    for p in panes::layout(tree, None, area, DIVIDER).panes {
                        let rect =
                            zoomed.iter().flatten().find(|z| z.block_id == p.block_id).map_or(p.rect, |z| z.rect);
                        if s.blocks.iter().any(|b| b.id == p.block_id) {
                            out.push((s.id.clone(), p.block_id.clone(), rect));
                        }
                    }
                }
            }
            out
        };
        for (sid, id, r) in fits {
            let (rows, cols) = cells_in(px(r.w - 2. * PADDING), px(r.h - 2. * PADDING), cell_w, line_h);
            self.app.fit(&sid, &id, rows, cols);
        }
        cx.notify();
    }

    pub(super) fn select(&mut self, session: &str, window: Option<usize>, cx: &mut Context<Self>) {
        self.session_id = Some(session.to_string());
        match window {
            Some(i) => self.focus_window(i, cx),
            None => cx.notify(),
        }
    }
}
