//! Motion PR 2: a split that comes while another is moving carries on from where the panes are,
//! and a pane whose size changed over both gets one SIGWINCH, when the motion ends.
mod layout_support;

use gpui::TestAppContext;
use layout_support::*;
use leon_app::layout_motion::LAYOUT;

#[gpui::test]
fn a_second_split_mid_motion_carries_on(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    ws.update(cx, |ws, _| ws.set_reduce_motion(false));
    let a = tab_running(&s, &ws, cx, &winches("a-ready"), "a-ready");
    to_rest(&ws, cx);
    // its first fit to its tile, before the splits, isn't theirs
    run_for(cx, std::time::Duration::from_millis(300));
    let before = winched(&s, &a);
    let sid = sid(&ws, cx);
    split_quiet(&s, &sid, &a, "n1-ready");
    assert!(moving(&ws, cx), "the first split moves");
    frame(&ws, cx, LAYOUT / 2);
    let mid = tile(cx, &a).unwrap();
    split_quiet(&s, &sid, &a, "n2-ready");
    // the second split arrives by an event, the clock still: wait for its motion to take over
    let order = |cx: &mut gpui::VisualTestContext| ws.read_with(cx, |ws, _| ws.tile_order().len());
    assert!(shows(cx, |cx| order(cx) == 3), "the second split is drawn");
    frame(&ws, cx, FRAME / 16);
    let after = tile(cx, &a).unwrap();
    to_rest(&ws, cx);
    let rest = tile(cx, &a).unwrap();
    let order = ws.read_with(cx, |ws, _| ws.tile_order());
    let widths: Vec<f32> = order.iter().map(|id| tile(cx, id).unwrap().2).collect();
    let one = shows(cx, |_| winched(&s, &a) - before == 1);
    run_for(cx, std::time::Duration::from_millis(500));
    let n = winched(&s, &a) - before;
    s.app.stop();
    let travel = (mid.2 - rest.2).abs();
    assert!(travel > 10., "it was moving: {mid:?} -> {rest:?}");
    assert!((after.2 - mid.2).abs() < travel * 0.1, "the second split carried on from where it was: {mid:?} {after:?}");
    assert!(one && n == 1, "one SIGWINCH for the pane over both splits, not {n}");
    assert_eq!(order.len(), 3, "three panes at rest");
    // a split twice: a and n2 share a's half (quarters), n1 keeps the other half
    let (q1, q2, half) = (widths[0], widths[1], widths[2]);
    assert!((q1 - q2).abs() < 2. && (half - (q1 + q2)).abs() < 12., "all end at the final layout: {widths:?}");
}
