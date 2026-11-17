//! Motion PR 2 (ADR R3): a layout motion re-renders only the panes that move: a pane whose rect
//! doesn't change keeps its cached grid for the whole motion.
mod layout_support;

use gpui::TestAppContext;
use layout_support::*;
use serde_json::json;

#[gpui::test]
fn a_pane_that_doesnt_move_doesnt_render(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    ws.update(cx, |ws, _| ws.set_reduce_motion(false));
    let a = tab_running(&s, &ws, cx, &quiet("a-ready"), "a-ready");
    let b = split_running(&s, &ws, cx, &a, &quiet("b-ready"), "b-ready");
    to_rest(&ws, cx);
    let a_tile = tile(cx, &a).unwrap();
    // b split below: a, on the left, keeps its rect
    let sid = sid(&ws, cx);
    let layout = json!({ "block": { "options": { "command": ["sh", "-c", quiet("c-ready")] } } });
    let p = json!({ "session_id": sid, "anchor_id": b, "direction": "vertical", "side": "after",
                    "layout": layout, "focus": false });
    s.app.call("session.new_split", p).unwrap();
    assert!(moving(&ws, cx), "the split moves");
    // from the motion's first frame: the split's own frame renders a's grid once with Reduce Motion
    // too (the layout changed), which isn't the motion's cost
    let (a_renders, b_renders) = (grid_renders(&ws, cx, &a), grid_renders(&ws, cx, &b));
    let n = to_rest(&ws, cx);
    let (a_after, b_after) = (grid_renders(&ws, cx, &a) - a_renders, grid_renders(&ws, cx, &b) - b_renders);
    let a_rest = tile(cx, &a).unwrap();
    s.app.stop();
    assert_eq!(a_rest, a_tile, "a didn't move");
    assert_eq!(a_after, 0, "a's grid didn't render over the {n} frames of the motion");
    assert!(b_after >= 1, "b's grid, which moved, did");
}
