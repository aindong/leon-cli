//! App update criterion 17 (.agent/plans/2026-11-16-app-update-notice.md): a notice on screen
//! doesn't make an idle window draw: what the poll compares to decide on a redraw stays the same.

mod m10b_support;

use gpui::TestAppContext;
use leon_app::updates::{How, Phase};
use m10b_support::*;

#[gpui::test]
fn a_shown_notice_leaves_the_window_idle(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let phase = Phase::Available { version: "9.9.9".into(), notes: "https://example.com/n".into(), how: How::Install };
    ws.read_with(cx, |ws, _| ws.updater().set_phase(phase));
    assert!(shows(cx, |cx| drawn(cx, "update-notice")), "the notice is drawn");
    let first = ws.read_with(cx, |ws, _| ws.signature());
    let mut later = Vec::new();
    for _ in 0..10 {
        cx.executor().advance_clock(std::time::Duration::from_millis(100));
        cx.run_until_parked();
        later.push(ws.read_with(cx, |ws, _| ws.signature()));
    }
    s.app.stop();
    assert!(later.iter().all(|s| *s == first), "the signature moved while nothing changed: {first} then {later:?}");
}

#[gpui::test]
fn a_new_phase_does_move_it(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let before = ws.read_with(cx, |ws, _| ws.signature());
    ws.read_with(cx, |ws, _| ws.updater().set_phase(Phase::Done { version: "9.9.9".into() }));
    let after = ws.read_with(cx, |ws, _| ws.signature());
    s.app.stop();
    assert_ne!(before, after, "a change to the notice is drawn");
}
