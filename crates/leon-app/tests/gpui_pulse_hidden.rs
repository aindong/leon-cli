//! ADR R6 (docs/architecture/gpui.md), phase 3: a window that draws no frames (hidden, covered)
//! shows no spinner, so the Pulse waits after two ticks for a drawn frame to go on.

use std::time::Duration;

use gpui::{AppContext, TestAppContext};
use leon_app::view::Pulse;

#[gpui::test]
fn the_pulse_waits_for_a_frame_after_two_undrawn_ticks(cx: &mut TestAppContext) {
    // no window: nothing draws, as for a hidden one
    let p = cx.new(|_| Pulse::new());
    p.update(cx, |p, cx| p.set_running(true, cx));
    for _ in 0..6 {
        cx.executor().advance_clock(Duration::from_millis(130));
        cx.run_until_parked();
    }
    let (running, ticks) = p.read_with(cx, |p, _| (p.running(), p.ticks()));
    assert_eq!((running, ticks), (false, 2), "two ticks, then it waits");
    p.update(cx, |p, cx| p.drew(true, cx));
    cx.executor().advance_clock(Duration::from_millis(130));
    cx.run_until_parked();
    let (running, after) = p.read_with(cx, |p, _| (p.running(), p.ticks()));
    assert!(running && after == ticks + 1, "a drawn frame starts it again: {after}");
    p.update(cx, |p, cx| p.set_running(false, cx));
    assert!(!p.read_with(cx, |p, _| p.running()), "and it stops when nothing works");
}
