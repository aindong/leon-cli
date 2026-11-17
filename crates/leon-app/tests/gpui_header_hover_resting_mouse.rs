//! A pane that comes under a resting mouse shows its header's controls before the mouse moves:
//! GPUI sends a hover only on a move, so when the panes move the one under the mouse is hovered.
mod grid_support;

use gpui::{point, Modifiers, MouseButton, TestAppContext, VisualTestContext};
use grid_support::*;
use serde_json::json;

#[gpui::test]
fn a_pane_split_in_under_a_resting_mouse_shows_working_controls(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let a = tab_running(&s, &ws, cx, &quiet("a-ready"), "a-ready");
    let tile = at(cx, &format!("pane-{a}")).expect("the pane's tile");
    // the mouse rests over the left quarter of the only pane
    let rest = point(tile.origin.x + tile.size.width * 0.25, tile.origin.y + tile.size.height / 2.);
    cx.simulate_mouse_move(rest, None::<MouseButton>, Modifiers::none());
    cx.run_until_parked();
    // an agent splits a pane in on the left, unfocused: it comes under the resting mouse
    let sid = ws.read_with(cx, |ws, _| ws.session_id()).unwrap();
    let layout = json!({ "block": { "options": { "command": ["sh", "-c", quiet("n-ready")] } } });
    let p = json!({ "session_id": sid, "anchor_id": a, "direction": "horizontal", "side": "before",
                    "layout": layout, "focus": false });
    let n = s.app.call("session.new_split", p).unwrap()["block_id"].as_str().unwrap().to_string();
    let drew = |cx: &mut VisualTestContext| {
        ws.read_with(cx, |ws, _| ws.grid_drew(&n)).is_some_and(|d| d.text.contains("n-ready"))
    };
    assert!(shows(cx, drew), "the new pane drew");
    assert_eq!(ws.read_with(cx, |ws, _| ws.target_block()), Some(a.clone()), "the old pane keeps the focus");
    let nt = at(cx, &format!("pane-{n}")).expect("the new pane's tile");
    let at_ = at(cx, &format!("pane-{a}")).expect("the old pane's tile");
    assert!(nt.contains(&rest), "the new pane is under the mouse");
    // its close control sits as far from its tile's right edge as the focused pane's does
    let a_close = center(at(cx, &format!("header-close-{a}")).expect("the focused pane's close control"));
    let close = point(nt.origin.x + nt.size.width - (at_.origin.x + at_.size.width - a_close.x), a_close.y);
    // a click there, the mouse not moved since the split: a hidden control takes no click
    cx.simulate_click(close, Modifiers::none());
    let closed = shows(cx, |cx| ws.read_with(cx, |ws, _| ws.pane_title(&n)).is_none());
    s.app.stop();
    assert!(closed, "the pane under the mouse showed its close control, and a click closed it");
}
