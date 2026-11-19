//! Review: a hover card on a float's link closes when the mouse moves off the link inside the
//! float (a float blocks the mouse from the window's root).
mod link_card_support;

use gpui::{Modifiers, MouseButton, TestAppContext};
use link_card_support::*;
use serde_json::json;

#[gpui::test]
fn leaving_a_link_in_a_float_closes_the_card(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let sid = ws.read_with(cx, |ws, _| ws.session_id()).unwrap();
    let layout = json!({ "block": { "options": { "command": ["sh", "-c", LINKS] } } });
    let bounds = json!({ "x": 0.1, "y": 0.1, "width": 0.8, "height": 0.8 });
    let r =
        s.app.call("session.new_floating", json!({ "session_id": sid, "bounds": bounds, "layout": layout })).unwrap();
    let id = r["block_id"].as_str().unwrap().to_string();
    let shown =
        shows(cx, |cx| ws.read_with(cx, |ws, _| ws.view_text(&id)).unwrap_or_default().contains("file:///etc/hosts"));
    assert!(shown, "the float never showed the links");
    let row = row_of(&ws, cx, &id, "go https://example.com/page here");
    let (web, off) = (cell(&ws, cx, &id, row, 8), cell(&ws, cx, &id, row, 1));
    rest(&ws, cx, web, Modifiers::none(), 500);
    assert!(card(&ws, cx).is_some(), "setup: the card opens on the float's link");
    cx.simulate_mouse_move(off, None::<MouseButton>, Modifiers::none());
    cx.run_until_parked();
    let left = card(&ws, cx);
    s.app.stop();
    assert_eq!(left, None, "off the link in the float: closed");
}
