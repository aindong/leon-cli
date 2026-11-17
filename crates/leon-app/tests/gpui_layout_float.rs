//! Motion PR 2, review: a tile floated while others are tiled is drawn once a frame (as a float),
//! never also as a tile leaving the layout: GPUI can't draw one cached view twice in a frame.
mod layout_support;

use gpui::TestAppContext;
use layout_support::*;
use serde_json::json;

#[gpui::test]
fn a_tile_floated_is_drawn_once_a_frame(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    ws.update(cx, |ws, _| ws.set_reduce_motion(false));
    let a = tab_running(&s, &ws, cx, &quiet("a-ready"), "a-ready");
    let b = split_running(&s, &ws, cx, &a, &quiet("b-ready"), "b-ready");
    to_rest(&ws, cx);
    let sid = sid(&ws, cx);
    let bounds = json!({ "x": 0.3, "y": 0.3, "width": 0.4, "height": 0.4 });
    s.app.call("session.float_block", json!({ "session_id": sid, "block_id": b, "bounds": bounds })).unwrap();
    assert!(shows(cx, |cx| drawn(cx, &format!("float-{b}"))), "b floats");
    let mut twice = Vec::new();
    loop {
        let d = leon_app::view::drawn();
        if d.contains(&format!("pane-{b}")) && d.contains(&format!("float-{b}")) {
            twice.push(ws.read_with(cx, |ws, _| ws.tile_order()));
        }
        if !ws.read_with(cx, |ws, _| ws.frame_asked()) {
            break;
        }
        frame(&ws, cx, FRAME);
    }
    s.app.stop();
    assert!(twice.is_empty(), "b drawn as a tile and a float in one frame: {twice:?}");
}
