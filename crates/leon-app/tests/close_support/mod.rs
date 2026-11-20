//! What the close and rename tests share: the M11 helpers, a tab running `sleep` (a program the
//! close asks about), and the server's count of tabs and sessions.
#![allow(dead_code)]

#[path = "../m11_support/mod.rs"]
mod m11;
pub use m11::*;

use gpui::{Entity, Modifiers, MouseButton, MouseDownEvent, Pixels, Point, VisualTestContext};
use leon_app::view::Workspace;
use serde_json::{json, Value};

/// A second tab in the workspace's session with `sleep` in front, once the app knows it runs;
/// returns (session, its window).
pub fn busy_tab(s: &Setup, ws: &Entity<Workspace>, cx: &mut VisualTestContext) -> (String, String) {
    tab_running(s, ws, cx, "echo busy-ready; exec sleep 100", "busy-ready");
    let sid = ws.read_with(cx, |ws, _| ws.session_id()).unwrap();
    let wid = window(&s.app, ws, cx).0.id;
    let known = shows(cx, |cx| ws.read_with(cx, |ws, _| ws.running_programs(&sid, Some(&wid))) == ["sleep"]);
    assert!(known, "the app never heard sleep runs");
    (sid, wid)
}

/// Wait until the app has heard every pane of session `sid` is at its shell: until the server's
/// first look a pane's program is unnamed, which a close counts as running.
pub fn shells_only(ws: &Entity<Workspace>, cx: &mut VisualTestContext, sid: &str) {
    let quiet = shows(cx, |cx| ws.read_with(cx, |ws, _| ws.running_programs(sid, None)).is_empty());
    assert!(quiet, "the app never heard the panes are at their shells");
}

/// Session `sid`'s label on the server.
pub fn label(s: &Setup, sid: &str) -> Value {
    let list = s.app.call("session.list", json!({})).unwrap();
    let found = list["sessions"].as_array().unwrap().iter().find(|x| x["session_id"] == sid).cloned();
    found.map_or(Value::Null, |x| x["label"].clone())
}

/// How many tabs session `sid` has on the server (0 once it is gone).
pub fn tabs(s: &Setup, sid: &str) -> usize {
    let list = s.app.call("session.list", json!({})).unwrap();
    let found = list["sessions"].as_array().unwrap().iter().find(|x| x["session_id"] == sid).cloned();
    found.map_or(0, |x| x["windows"].as_u64().unwrap() as usize)
}

pub fn sessions(s: &Setup) -> usize {
    s.app.call("session.list", json!({})).unwrap()["sessions"].as_array().unwrap().len()
}

pub fn double_click(cx: &mut VisualTestContext, at: Point<Pixels>) {
    for n in 1..=2 {
        cx.simulate_event(MouseDownEvent {
            button: MouseButton::Left,
            position: at,
            modifiers: Modifiers::none(),
            click_count: n,
            first_mouse: false,
        });
        cx.simulate_mouse_up(at, MouseButton::Left, Modifiers::none());
    }
    cx.run_until_parked();
}

pub fn right_click(cx: &mut VisualTestContext, at: Point<Pixels>) {
    cx.simulate_mouse_down(at, MouseButton::Right, Modifiers::none());
    cx.simulate_mouse_up(at, MouseButton::Right, Modifiers::none());
    cx.run_until_parked();
}

/// Rest the mouse on the element drawn as `row`, then click the one drawn as `button` (shown on hover).
pub fn hover_click(cx: &mut VisualTestContext, row: &str, button: &str) {
    let p = center(at(cx, row).unwrap_or_else(|| panic!("{row} drawn")));
    cx.simulate_mouse_move(p, None, Modifiers::none());
    let p = center(at(cx, button).unwrap_or_else(|| panic!("{button} drawn")));
    cx.simulate_mouse_move(p, None, Modifiers::none());
    // read again: a title the program set since can have moved the button
    let p = center(at(cx, button).unwrap_or_else(|| panic!("{button} drawn")));
    cx.simulate_click(p, Modifiers::none());
    cx.run_until_parked();
}
