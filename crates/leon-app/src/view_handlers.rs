//! The workspace's actions and input: splits, focus, divider drags, tabs, paste and keys.

use super::*;

impl Workspace {
    /// Split the target pane; a zoom ends first so the new pane shows.
    pub(super) fn split(&mut self, direction: Direction, cx: &mut Context<Self>) {
        let Some((_, id, zoomed)) = self.target() else { return };
        if zoomed {
            self.call("session.zoom_block", json!({ "block_id": id, "zoom": false }), cx);
        }
        let p = json!({ "anchor_id": id, "direction": direction, "side": "after", "layout": { "block": {} }, "focus": true });
        self.call("session.new_split", p, cx);
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
        self.split(Direction::Horizontal, cx);
    }

    pub(super) fn split_down(&mut self, _: &SplitDown, _: &mut Window, cx: &mut Context<Self>) {
        self.split(Direction::Vertical, cx);
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
        let st = self.app.ctx().state();
        let s = st.session(&sid).ok()?;
        let wid = s.windows.get(index.unwrap_or(s.active))?.id.clone();
        Some((sid, wid))
    }

    pub(super) fn focus_window(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some((sid, wid)) = self.window_ref(Some(index)) {
            let _ = self.app.call("session.focus_window", json!({ "session_id": sid, "window_id": wid }));
            cx.notify();
        }
    }

    /// Close a window; if that ended its session, select a neighbouring one.
    pub(super) fn close_window(&mut self, (sid, wid): (String, String), cx: &mut Context<Self>) {
        let _ = self.app.call("session.close_window", json!({ "session_id": sid, "window_id": wid }));
        let rows = self.rows();
        self.settle(&rows);
        cx.notify();
    }

    pub(super) fn step_tab(&mut self, by: isize, cx: &mut Context<Self>) {
        let Some(sid) = self.session_id.clone() else { return };
        let Some((n, active)) = self.app.ctx().state().session(&sid).ok().map(|s| (s.windows.len(), s.active)) else {
            return;
        };
        if n > 0 {
            self.focus_window((active as isize + by).rem_euclid(n as isize) as usize, cx);
        }
    }

    pub(super) fn new_tab(&mut self, _: &NewTab, window: &mut Window, cx: &mut Context<Self>) {
        match self.session_id.clone() {
            Some(sid) => drop(self.app.call("session.new_window", json!({ "session_id": sid }))),
            None => self.new_session(&NewSession, window, cx),
        }
        cx.notify();
    }

    pub(super) fn new_session(&mut self, _: &NewSession, _: &mut Window, cx: &mut Context<Self>) {
        if let Ok(r) = self.app.call("session.create", json!({})) {
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
        let (Some(t), Some(text)) = (self.terminal(), cx.read_from_clipboard().and_then(|c| c.text())) else {
            return;
        };
        let bracketed = t.with_term(|term| term.mode().contains(TermMode::BRACKETED_PASTE));
        let _ = t.write(&crate::input::paste_bytes(&text, bracketed));
    }

    pub(super) fn toggle_sidebar(&mut self, _: &ToggleSidebar, _: &mut Window, cx: &mut Context<Self>) {
        self.sidebar = !self.sidebar;
        cx.notify();
    }

    pub(super) fn key_down(&mut self, ev: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let ks = &ev.keystroke;
        let m = &ks.modifiers;
        if m.platform {
            let digit = ks.key.parse::<usize>().ok().filter(|d| (1..=9).contains(d));
            if let Some(d) = digit.filter(|_| !(m.control || m.alt || m.shift)) {
                self.focus_window(d - 1, cx);
                cx.stop_propagation();
            }
            return;
        }
        let Some(t) = self.terminal() else { return };
        let app_cursor = t.with_term(|term| term.mode().contains(TermMode::APP_CURSOR));
        if let Some(bytes) = crate::input::key_bytes(ks, app_cursor) {
            let _ = t.write(&bytes);
            cx.stop_propagation();
        }
    }

    pub(super) fn select(&mut self, session: &str, window: Option<usize>, cx: &mut Context<Self>) {
        self.session_id = Some(session.to_string());
        match window {
            Some(i) => self.focus_window(i, cx),
            None => cx.notify(),
        }
    }
}
