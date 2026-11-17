//! Motion PR 2: switching tabs cross-fades: the panes area shows under a veil in the chrome colour
//! that fades out over `SWITCH`; switching back mid-fade carries on from where the veil is.
mod layout_support;

use gpui::TestAppContext;
use layout_support::*;
use leon_app::layout_motion::SWITCH;
use serde_json::json;

#[gpui::test]
fn a_tab_switch_fades_in_under_a_veil(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    ws.update(cx, |ws, _| ws.set_reduce_motion(false));
    let first = window(&s.app, &ws, cx).0.id;
    let a = tab_running(&s, &ws, cx, &quiet("a-ready"), "a-ready");
    to_rest(&ws, cx);
    let second = window(&s.app, &ws, cx).0.id;
    let sid = sid(&ws, cx);
    let veil = |cx: &mut gpui::VisualTestContext| ws.read_with(cx, |ws, _| ws.veil());
    let focus = |w: &str| s.app.call("session.focus_window", json!({ "session_id": sid, "window_id": w })).unwrap();
    assert_eq!(veil(cx), 0., "no veil at rest");
    focus(&first);
    assert!(moving(&ws, cx), "the switch fades");
    let v0 = veil(cx);
    frame(&ws, cx, SWITCH / 2);
    let v1 = veil(cx);
    focus(&second); // back, mid-fade
    assert!(shows(cx, |cx| ws.read_with(cx, |ws, _| ws.target_block()) == Some(a.clone())), "back on a's tab");
    let v2 = veil(cx);
    let drawn_veil = drawn(cx, "panes-veil");
    frame(&ws, cx, FRAME);
    let v2b = veil(cx);
    let n = to_rest(&ws, cx);
    let v3 = veil(cx);
    s.app.stop();
    assert!(v0 > 0.9, "the first frame is under the veil: {v0}");
    assert!(v1 > 0. && v1 < v0, "half way it is fading: {v1}");
    assert!((v2 - v1).abs() < 0.01, "a switch back mid-fade carries on from where the veil is: {v1} -> {v2}");
    assert!(v2b > 0. && v2b < v2, "and keeps fading: {v2} -> {v2b}");
    assert!(drawn_veil, "the veil is drawn over the panes area");
    assert_eq!(v3, 0., "then it is gone ({n} frames)");
}
