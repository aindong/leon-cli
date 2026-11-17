//! Motion PR 1 (ADR R6): with Reduce Motion, a drawer shows and hides and a fade lands at once:
//! the first frame is the end state, and the app asks GPUI for no frame in between.
mod m20b_support;

use gpui::TestAppContext;
use m20b_support::*;
use serde_json::json;

#[gpui::test]
fn reduce_motion_draws_no_frame_in_between(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    ws.update(cx, |ws, _| ws.set_reduce_motion(true));
    let sid = sid(&ws, cx);
    let toggle = |create: bool| {
        s.app.call("session.toggle", json!({ "session_id": sid, "label": "scratch", "create": create })).unwrap();
    };
    toggle(true);
    let id = view_window(&s.app, &sid)["docked"][0]["block_id"].as_str().unwrap().to_string();
    let sel = format!("dock-{id}");
    assert!(shows(cx, |cx| drawn(cx, &sel)), "the drawer is drawn");
    let still = |cx: &mut gpui::VisualTestContext| ws.read_with(cx, |ws, _| (ws.animating(), ws.frame_asked()));
    let shown = still(cx);
    let first = at(cx, &sel).unwrap();
    // a later frame, the clock moved on: the drawer was already where it rests
    cx.executor().advance_clock(leon_app::floats::SLIDE * 2);
    ws.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();
    let rest = at(cx, &sel).unwrap();
    toggle(false);
    assert!(shows(cx, |cx| !drawn(cx, &sel)), "hidden at once");
    let hidden = still(cx);

    let pane = ws.read_with(cx, |ws, _| ws.target_block()).unwrap();
    act(&ws, cx, "pane.split.right");
    assert!(shows(cx, |cx| ws.read_with(cx, |ws, _| ws.target_block()).is_some_and(|t| t != pane)), "split");
    let faded = ws.read_with(cx, |ws, _| ws.pane_fade(&pane));
    let split = still(cx);
    s.app.stop();
    assert_eq!(first, rest, "drawn at rest from its first frame");
    assert_eq!(shown, (false, false), "showing it moved nothing");
    assert_eq!(hidden, (false, false), "hiding it moved nothing");
    assert_eq!(faded, leon_lua::settings::FADE as f32, "the pane that lost the focus is faded at once");
    assert_eq!(split, (false, false), "and nothing moves");
}
