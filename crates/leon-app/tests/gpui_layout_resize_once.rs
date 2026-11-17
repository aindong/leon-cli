//! Motion PR 2: a pane's PTY is resized once, when its motion ends: its program gets no SIGWINCH
//! while the panes move (its grid keeps its size, clipped by the moving tile) and one after.
mod layout_support;

use gpui::TestAppContext;
use layout_support::*;
use leon_app::layout_motion::LAYOUT;

#[gpui::test]
fn a_split_resizes_the_old_pane_once_at_the_end(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    ws.update(cx, |ws, _| ws.set_reduce_motion(false));
    let a = tab_running(&s, &ws, cx, &winches("a-ready"), "a-ready");
    to_rest(&ws, cx);
    run_for(cx, std::time::Duration::from_millis(300));
    let before = winched(&s, &a);
    let sid = sid(&ws, cx);
    split_quiet(&s, &sid, &a, "n-ready");
    assert!(moving(&ws, cx), "the split moves");
    for _ in 0..4 {
        frame(&ws, cx, LAYOUT / 5);
    }
    let during = winched(&s, &a) - before;
    to_rest(&ws, cx);
    let one = shows(cx, |_| winched(&s, &a) - before == 1);
    run_for(cx, std::time::Duration::from_millis(500));
    let after = winched(&s, &a) - before;
    s.app.stop();
    assert_eq!(during, 0, "no SIGWINCH while the panes move");
    assert!(one && after == 1, "one SIGWINCH when the motion ends, not {after}");
}
