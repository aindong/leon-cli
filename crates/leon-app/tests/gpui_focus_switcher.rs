//! GPUI phase 6 (ADR R9): the session switcher (the palette's other list) takes the focus while
//! open; Escape, its key again, and a jump to another session's tab each give the focus back to
//! the pane shown, and typed text reaches it.
mod focus_support;

use focus_support::*;
use gpui::TestAppContext;
use serde_json::json;

fn open_switcher(ws: &gpui::Entity<leon_app::view::Workspace>, cx: &mut gpui::VisualTestContext) {
    cx.simulate_keystrokes("cmd-k");
    assert!(ws.read_with(cx, |ws, _| ws.switcher_open()), "the switcher opened");
    assert_eq!(owner(ws, cx), Some("palette"), "the switcher has the focus");
}

#[gpui::test]
fn every_way_the_switcher_closes_gives_the_pane_its_keys(cx: &mut TestAppContext) {
    let s = plain_setup();
    let (ws, cx) = open(cx, &s.app);
    let first = ws.read_with(cx, |ws, _| ws.session_id()).unwrap();
    let other = s.app.call("session.create", json!({ "label": "deploys" })).unwrap();
    let other = other["session_id"].as_str().unwrap().to_string();
    settle(&focused(&s.app, &ws, cx));

    open_switcher(&ws, cx);
    cx.simulate_keystrokes("escape");
    back_on_pane(&s, &ws, cx, "esc1").unwrap();

    open_switcher(&ws, cx);
    cx.simulate_keystrokes("cmd-k");
    back_on_pane(&s, &ws, cx, "toggle2").unwrap();

    let listed = |cx: &mut gpui::VisualTestContext| {
        ws.read_with(cx, |ws, _| ws.switcher_rows()).iter().any(|r| r.contains("deploys"))
    };
    open_switcher(&ws, cx);
    assert!(shows(cx, listed), "the other session is listed");
    cx.simulate_keystrokes("d e p l o y s enter");
    assert!(shows(cx, |cx| ws.read_with(cx, |ws, _| ws.session_id()).as_deref() == Some(other.as_str())));
    assert_ne!(first, other);
    let r = back_on_pane(&s, &ws, cx, "jump3");
    s.app.stop();
    r.unwrap();
}
