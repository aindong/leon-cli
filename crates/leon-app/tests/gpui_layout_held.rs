//! Motion PR 2, review (CodeRabbit): a pane a split is still making keeps the size its PTY got when
//! a second change comes mid-motion, so it isn't resized before the second motion ends.
mod layout_support;

use gpui::TestAppContext;
use layout_support::*;
use leon_app::layout_motion::LAYOUT;

#[gpui::test]
fn a_new_pane_keeps_its_size_through_a_second_change(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    ws.update(cx, |ws, _| ws.set_reduce_motion(false));
    let a = tab_running(&s, &ws, cx, &quiet("a-ready"), "a-ready");
    to_rest(&ws, cx);
    let sid = sid(&ws, cx);
    let n = split_quiet(&s, &sid, &a, "n-ready");
    assert!(moving(&ws, cx), "the split moves");
    frame(&ws, cx, LAYOUT / 2);
    let held = ws.read_with(cx, |ws, _| ws.held_size(&n)).expect("the new pane is held while it moves");
    split_quiet(&s, &sid, &n, "m-ready"); // the new pane split again, mid-motion
    let order = |cx: &mut gpui::VisualTestContext| ws.read_with(cx, |ws, _| ws.tile_order().len());
    assert!(shows(cx, |cx| order(cx) == 3), "the second split is drawn");
    let after = ws.read_with(cx, |ws, _| ws.held_size(&n));
    to_rest(&ws, cx);
    let rest = ws.read_with(cx, |ws, _| ws.held_size(&n));
    s.app.stop();
    assert_eq!(after, Some(held), "it keeps the size its PTY got through the second change");
    assert_eq!(rest, None, "and takes its new size when the motion ends");
}
