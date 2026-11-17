//! Motion PR 1 (ADR R6): the poll keeps no animation clock. While a fade moves, the frames it
//! draws are the ones the app asks GPUI for; the poll doesn't run every 8 ms to redraw.
mod grid_support;

use std::time::Duration;

use gpui::TestAppContext;
use grid_support::*;

#[gpui::test]
fn a_fade_runs_no_poll_clock(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    ws.update(cx, |ws, _| ws.set_reduce_motion(false));
    let a = tab_running(&s, &ws, cx, &quiet("a-ready"), "a-ready");
    split_running(&s, &ws, cx, &a, &quiet("b-ready"), "b-ready");
    while ws.read_with(cx, |ws, _| ws.frame_asked()) {
        frame(&ws, cx, Duration::from_millis(16));
    }
    act(&ws, cx, "pane.focus.left");
    assert!(ws.read_with(cx, |ws, _| ws.fading()), "a fade moves");
    let polls = ws.read_with(cx, |ws, _| ws.polls().0);
    let mut frames = 0;
    while ws.read_with(cx, |ws, _| ws.frame_asked()) {
        frame(&ws, cx, Duration::from_millis(8));
        frames += 1;
        assert!(frames < 100, "the fade never ended");
    }
    let polls = ws.read_with(cx, |ws, _| ws.polls().0) - polls;
    s.app.stop();
    assert!(frames >= 5, "the fade drew {frames} frames");
    assert!(polls <= 2, "the poll ran {polls} times over {frames} frames: it keeps an animation clock");
}
