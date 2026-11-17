//! Motion PR 1 (ADR R6, R10): a drawer slides on GPUI's clock. Each step is a frame the app asked
//! for, drawn after advancing the test clock: no real-time wait.
mod m20b_support;

use std::time::Duration;

use gpui::{Entity, TestAppContext, VisualTestContext};
use leon_app::embedded;
use leon_app::floats::SLIDE;
use leon_app::view::Workspace;
use m20b_support::*;
use serde_json::json;

const FRAME: Duration = Duration::from_millis(16);

fn top(cx: &mut VisualTestContext, sel: &str) -> f32 {
    f32::from(at(cx, sel).expect("the drawer is drawn").origin.y)
}

/// Frames the app asks for, until it asks for none; how many.
fn to_rest(ws: &Entity<Workspace>, cx: &mut VisualTestContext) -> usize {
    let mut n = 0;
    while ws.read_with(cx, |ws, _| ws.frame_asked()) {
        frame(ws, cx, FRAME);
        n += 1;
        assert!(n < 100, "the slide never ended");
    }
    n
}

#[gpui::test]
fn a_drawer_slides_in_on_the_test_clock(cx: &mut TestAppContext) {
    let dir = private_dir();
    let app = std::rc::Rc::new(embedded::start_at(dir.path().join("server.sock")).unwrap());
    let (ws, cx) = open_with(cx, &app, None);
    ws.update(cx, |ws, _| ws.set_reduce_motion(false));
    let sid = sid(&ws, cx);
    app.call("session.toggle", json!({ "session_id": sid, "label": "scratch", "create": true })).unwrap();
    let id = view_window(&app, &sid)["docked"][0]["block_id"].as_str().unwrap().to_string();
    let sel = format!("dock-{id}");
    assert!(shows(cx, |cx| drawn(cx, &sel)), "the drawer is drawn");
    // the clock has not moved since it showed: this frame is the slide's first
    let start = top(cx, &sel);
    assert!(ws.read_with(cx, |ws, _| ws.sliding()), "it slides");
    frame(&ws, cx, SLIDE / 2);
    let half = top(cx, &sel);
    let frames = to_rest(&ws, cx);
    let rest = top(cx, &sel);
    app.stop();
    assert!(start > half && half > rest, "it rises from below its place: {start} > {half} > {rest}");
    assert!(frames >= 1, "it asked for frames until it was at rest");
    assert!(!ws.read_with(cx, |ws, _| ws.sliding()), "and the slide is over");
}
