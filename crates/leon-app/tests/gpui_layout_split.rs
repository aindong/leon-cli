//! Motion PR 2: a split moves the panes. The new pane grows from the edge it was split at while
//! its sibling gives up the space, on GPUI's clock, and both end at their place.
mod layout_support;

use gpui::TestAppContext;
use layout_support::*;
use leon_app::layout_motion::LAYOUT;

#[gpui::test]
fn a_split_grows_the_new_pane_from_the_edge(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    ws.update(cx, |ws, _| ws.set_reduce_motion(false));
    let a = tab_running(&s, &ws, cx, &quiet("a-ready"), "a-ready");
    to_rest(&ws, cx);
    let whole = tile(cx, &a).expect("a's tile");
    let sid = sid(&ws, cx);
    let n = split_quiet(&s, &sid, &a, "n-ready");
    assert!(moving(&ws, cx), "the split moves the panes");
    // the clock hasn't moved since the split arrived: this frame is the motion's first
    let (a0, n0) = (tile(cx, &a).unwrap(), tile(cx, &n).expect("the new pane's tile"));
    frame(&ws, cx, LAYOUT / 2);
    let (a1, n1) = (tile(cx, &a).unwrap(), tile(cx, &n).unwrap());
    let frames = to_rest(&ws, cx);
    let (a2, n2) = (tile(cx, &a).unwrap(), tile(cx, &n).unwrap());
    let asked = ws.read_with(cx, |ws, _| ws.frame_asked());
    s.app.stop();
    assert_eq!(a0, whole, "first frame: the old pane still fills the window");
    assert!(n0.2 < 1., "first frame: the new pane is a strip at the edge: {n0:?}");
    assert!((n0.0 - (whole.0 + whole.2)).abs() < 1., "at the old pane's right edge: {n0:?} {whole:?}");
    assert!(between_w(a0, a1, a2) && between_w(n0, n1, n2), "half way, both between: {a1:?} {n1:?}");
    assert!((a2.2 - n2.2).abs() < 1. && a2.2 < whole.2 / 2., "at rest, two halves: {a2:?} {n2:?}");
    assert!(frames >= 2 && !asked, "it asked for frames until it rested ({frames}), then none");
}
