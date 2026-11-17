//! Motion PR 2: with Reduce Motion every layout change draws its end on its first frame and asks
//! GPUI for no frame in between.
mod layout_support;

use gpui::TestAppContext;
use layout_support::*;
use serde_json::json;

#[gpui::test]
fn reduce_motion_moves_no_pane(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    ws.update(cx, |ws, _| ws.set_reduce_motion(true));
    let first = window(&s.app, &ws, cx).0.id;
    let a = tab_running(&s, &ws, cx, &quiet("a-ready"), "a-ready");
    let whole = tile(cx, &a).unwrap();
    let sid = sid(&ws, cx);
    let still = |cx: &mut gpui::VisualTestContext| ws.read_with(cx, |ws, _| (ws.animating(), ws.frame_asked()));
    let mut seen = Vec::new();

    let b = split_quiet(&s, &sid, &a, "b-ready");
    assert!(shows(cx, |cx| tile(cx, &b).is_some()), "the split is drawn");
    let halves = (tile(cx, &a).unwrap(), tile(cx, &b).unwrap());
    seen.push(("split", still(cx)));

    zoom(&s, &sid, &a, true);
    assert!(shows(cx, |cx| tile(cx, &b).is_none()), "the zoom is drawn");
    let zoomed = tile(cx, &a).unwrap();
    seen.push(("zoom", still(cx)));
    zoom(&s, &sid, &a, false);
    assert!(shows(cx, |cx| tile(cx, &b).is_some()), "the unzoom is drawn");
    seen.push(("unzoom", still(cx)));

    close(&s, &sid, &b);
    assert!(shows(cx, |cx| tile(cx, &b).is_none()), "the close is drawn");
    let alone = tile(cx, &a).unwrap();
    seen.push(("close", still(cx)));

    s.app.call("session.focus_window", json!({ "session_id": sid, "window_id": first })).unwrap();
    assert!(shows(cx, |cx| tile(cx, &a).is_none()), "the switch is drawn");
    let veil = ws.read_with(cx, |ws, _| ws.veil());
    seen.push(("switch", still(cx)));
    s.app.stop();

    assert!((halves.0 .2 - halves.1 .2).abs() < 1., "the split's first frame is its end: {halves:?}");
    assert_eq!(zoomed, whole, "the zoom's first frame fills the window");
    assert_eq!(alone, whole, "the close's first frame gives the space back");
    assert_eq!(veil, 0., "no veil");
    for (what, (animating, asked)) in seen {
        assert_eq!((animating, asked), (false, false), "{what}: nothing moves, no frame asked for");
    }
}
