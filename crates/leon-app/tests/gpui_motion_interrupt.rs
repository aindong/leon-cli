//! Motion PR 1 (ADR R6): a drawer hidden half way through sliding in turns back from where it is,
//! never jumping to its start or its end, and ends hidden.
mod m20b_support;

use std::time::Duration;

use gpui::{TestAppContext, VisualTestContext};
use leon_app::floats::SLIDE;
use m20b_support::*;
use serde_json::json;

#[gpui::test]
fn a_slide_turned_back_carries_on_from_where_it_is(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    ws.update(cx, |ws, _| ws.set_reduce_motion(false));
    let sid = sid(&ws, cx);
    let toggle = |create: bool| {
        s.app.call("session.toggle", json!({ "session_id": sid, "label": "scratch", "create": create })).unwrap();
    };
    toggle(true);
    let id = view_window(&s.app, &sid)["docked"][0]["block_id"].as_str().unwrap().to_string();
    let sel = format!("dock-{id}");
    assert!(shows(cx, |cx| drawn(cx, &sel)), "the drawer is drawn");
    let top = |cx: &mut VisualTestContext| f32::from(at(cx, &sel).unwrap().origin.y);
    let start = top(cx);
    frame(&ws, cx, SLIDE / 2);
    let mid = top(cx);
    toggle(false);
    // the hide arrives by an event: the frame it starts draws the drawer where it was
    assert!(shows(cx, |cx| ws.read_with(cx, |ws, _| ws.sliding()) && drawn(cx, &sel)), "it turns back");
    frame(&ws, cx, Duration::from_millis(1));
    let after = top(cx);
    let mut n = 0;
    while ws.read_with(cx, |ws, _| ws.frame_asked()) {
        frame(&ws, cx, Duration::from_millis(16));
        n += 1;
        assert!(n < 100, "the slide never ended");
    }
    let gone = !drawn(cx, &sel);
    s.app.stop();
    let travel = start - mid;
    assert!(travel > 10., "it had risen half way: {start} -> {mid}");
    assert!((after - mid).abs() < travel * 0.1, "it turned back from where it was: {mid} -> {after}");
    assert!(gone, "and ends hidden");
}
