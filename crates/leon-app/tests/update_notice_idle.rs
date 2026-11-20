//! App update criterion 17 (.agent/plans/2026-11-16-app-update-notice.md): a notice on screen
//! doesn't make an idle window draw: the root isn't rendered again while nothing changes.

mod m10b_support;

use gpui::{Entity, TestAppContext, VisualTestContext};
use leon_app::updates::{How, Phase};
use leon_app::view::Workspace;
use m10b_support::*;

fn setup() -> Setup {
    plain_setup()
}

/// Until the pane's shell has started and the app knows the program in front: the server
/// reports it on a 200 ms tick, and its arrival is a change the window draws.
fn shell_known(s: &Setup, ws: &Entity<Workspace>, cx: &mut VisualTestContext) {
    let id = ws.read_with(cx, |ws, _| ws.target_block()).unwrap();
    let t = term(&s.app, &id);
    run_until(&t, "echo ready", "ready");
    let known = |cx: &mut VisualTestContext| {
        let remote = ws.read_with(cx, |ws, _| ws.host_remote(leon_app::hosts::THIS_MAC)).unwrap();
        let shown = remote.model().block(&id).map(|(_, b)| b.foreground.clone());
        let now = t.foreground();
        now.is_some() && shown == Some(Some(now))
    };
    assert!(shows(cx, known), "the app heard of the pane's program: {:?}", t.foreground());
}

#[gpui::test]
fn a_shown_notice_leaves_the_window_idle(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    shell_known(&s, &ws, cx);
    let phase = Phase::Available { version: "9.9.9".into(), notes: "https://example.com/n".into(), how: How::Install };
    ws.read_with(cx, |ws, _| ws.updater().set_phase(phase));
    assert!(shows(cx, |cx| drawn(cx, "update-notice")), "the notice is drawn");
    let first = ws.read_with(cx, |ws, _| ws.renders());
    let mut later = Vec::new();
    for _ in 0..10 {
        cx.executor().advance_clock(std::time::Duration::from_millis(100));
        cx.run_until_parked();
        later.push(ws.read_with(cx, |ws, _| ws.renders()));
    }
    s.app.stop();
    assert!(later.iter().all(|s| *s == first), "the root rendered while nothing changed: {first} then {later:?}");
}

#[gpui::test]
fn a_new_phase_does_move_it(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let before = ws.read_with(cx, |ws, _| ws.root_redraws());
    ws.read_with(cx, |ws, _| ws.updater().set_phase(Phase::Done { version: "9.9.9".into() }));
    let moved = shows(cx, |cx| ws.read_with(cx, |ws, _| ws.root_redraws()) > before);
    s.app.stop();
    assert!(moved, "a change to the notice is drawn");
}
