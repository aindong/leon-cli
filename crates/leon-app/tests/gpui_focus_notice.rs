//! GPUI phase 6 (ADR R9): the update notice takes no focus (it isn't modal): typed text reaches
//! the pane while it shows, and after Dismiss.
mod focus_support;

use focus_support::*;
use gpui::{Modifiers, TestAppContext};
use leon_app::updates::{How, Phase};

#[gpui::test]
fn the_notice_never_takes_the_panes_keys(cx: &mut TestAppContext) {
    let s = plain_setup();
    let (ws, cx) = open(cx, &s.app);
    settle(&focused(&s.app, &ws, cx));
    let phase = Phase::Available { version: "9.9.9".into(), notes: "https://example.com/n".into(), how: How::Install };
    ws.read_with(cx, |ws, _| ws.updater().set_phase(phase));
    assert!(shows(cx, |cx| drawn(cx, "update-notice")), "the notice is drawn");
    back_on_pane(&s, &ws, cx, "shown1").unwrap();
    let dismiss = center(at(cx, "update-dismiss").expect("Dismiss"));
    cx.simulate_click(dismiss, Modifiers::none());
    assert!(shows(cx, |cx| !drawn(cx, "update-dismiss")), "dismissed");
    let r = back_on_pane(&s, &ws, cx, "gone2");
    s.app.stop();
    r.unwrap();
}
