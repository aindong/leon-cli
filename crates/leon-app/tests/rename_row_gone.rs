//! Review: when the tab being renamed goes away (closed from the CLI), the field lets go of the keys.
mod close_support;

use close_support::*;
use gpui::TestAppContext;
use serde_json::json;

#[gpui::test]
fn keys_are_freed_once_the_renamed_tab_is_gone(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    tab_running(&s, &ws, cx, "echo second-ready; exec sh", "second-ready");
    let sid = ws.read_with(cx, |ws, _| ws.session_id()).unwrap();
    let wid = window(&s.app, &ws, cx).0.id;
    let p = center(at(cx, "row-0-1").expect("the second tab row"));
    double_click(cx, p);
    assert!(ws.read_with(cx, |ws, _| ws.renaming()).is_some(), "the field opens");
    s.app.call("session.close_window", json!({ "session_id": sid, "window_id": wid })).unwrap();
    assert!(shows(cx, |cx| !drawn(cx, "row-0-1")), "the tab left the sidebar");
    cx.simulate_keystrokes("x");
    let freed = ws.read_with(cx, |ws, _| ws.renaming()).is_none();
    s.app.stop();
    assert!(freed, "the rename of a tab that is gone still takes every key");
}
