//! What the M11 view tests share: the M10b helpers, a tab running a script, and cell positions.
#![allow(dead_code)]

#[path = "../m10b_support/mod.rs"]
mod m10b;
pub use m10b::*;

use gpui::{Entity, Modifiers, MouseButton, MouseDownEvent, MouseUpEvent, Pixels, Point, VisualTestContext};
use leon_app::view::Workspace;
use serde_json::json;

/// Open a tab in the workspace's session running `sh -c script`; returns its block once the app
/// shows it and its screen has `ready` on it (the tab before it may show `ready` too).
pub fn tab_running(s: &Setup, ws: &Entity<Workspace>, cx: &mut VisualTestContext, script: &str, ready: &str) -> String {
    let sid = ws.read_with(cx, |ws, _| ws.session_id()).unwrap();
    let layout = json!({ "block": { "options": { "command": ["sh", "-c", script] } } });
    let tab = s.app.call("session.new_window", json!({ "session_id": sid, "layout": layout })).unwrap();
    let id = tab["block_id"].as_str().unwrap().to_string();
    let shown = shows(cx, |cx| {
        let target = ws.read_with(cx, |ws, _| ws.target_block());
        let text = ws.read_with(cx, |ws, _| ws.view_text(&id)).unwrap_or_default();
        target.as_ref() == Some(&id) && text.contains(ready)
    });
    assert!(shown, "the tab never showed {ready:?}");
    id
}

/// The lines the pane shows now (scrolled or not), trailing blanks dropped.
pub fn view_lines(ws: &Entity<Workspace>, cx: &mut VisualTestContext, id: &str) -> Vec<String> {
    let text = ws.read_with(cx, |ws, _| ws.view_text(id)).unwrap_or_default();
    text.lines().map(|l| l.trim_end().to_string()).collect()
}

/// Which row of the view shows `text` on a line of its own.
pub fn row_of(ws: &Entity<Workspace>, cx: &mut VisualTestContext, id: &str, text: &str) -> usize {
    let lines = view_lines(ws, cx, id);
    lines.iter().position(|l| l == text).unwrap_or_else(|| panic!("no row shows {text:?}: {lines:?}"))
}

/// The middle of cell (row, col) of the pane's view, in window coordinates.
pub fn cell(ws: &Entity<Workspace>, cx: &mut VisualTestContext, id: &str, row: usize, col: usize) -> Point<Pixels> {
    cx.run_until_parked();
    ws.read_with(cx, |ws, _| ws.cell_position(id, row, col)).expect("the pane was painted")
}

/// One line's height in the pane.
pub fn line_height(ws: &Entity<Workspace>, cx: &mut VisualTestContext, id: &str) -> Pixels {
    cell(ws, cx, id, 1, 0).y - cell(ws, cx, id, 0, 0).y
}

pub fn press(cx: &mut VisualTestContext, at: Point<Pixels>, modifiers: Modifiers, click_count: usize) {
    cx.simulate_event(MouseDownEvent {
        button: MouseButton::Left,
        position: at,
        modifiers,
        click_count,
        first_mouse: false,
    });
}

pub fn release(cx: &mut VisualTestContext, at: Point<Pixels>, modifiers: Modifiers, click_count: usize) {
    cx.simulate_event(MouseUpEvent { button: MouseButton::Left, position: at, modifiers, click_count });
}

/// Press at `from`, move to `to` with the button held, release there.
pub fn drag(cx: &mut VisualTestContext, from: Point<Pixels>, to: Point<Pixels>, modifiers: Modifiers) {
    press(cx, from, modifiers, 1);
    cx.simulate_mouse_move(to, MouseButton::Left, modifiers);
    release(cx, to, modifiers, 1);
    cx.run_until_parked();
}

pub fn clipboard(cx: &mut VisualTestContext) -> Option<String> {
    cx.read_from_clipboard().and_then(|c| c.text())
}

pub fn cmd() -> Modifiers {
    Modifiers { platform: true, ..Modifiers::none() }
}

pub fn shift() -> Modifiers {
    Modifiers { shift: true, ..Modifiers::none() }
}

pub fn alt() -> Modifiers {
    Modifiers { alt: true, ..Modifiers::none() }
}
