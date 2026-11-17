//! A working indicator that nothing on screen shows doesn't redraw the window at the pulse rate.
mod m9_support;

use std::time::Duration;

use gpui::TestAppContext;
use leon_app::status::Indicator;
use m9_support::*;

#[gpui::test]
fn a_spinner_nothing_shows_does_not_redraw(cx: &mut TestAppContext) {
    let s = plain_setup(); // a login shell's prompt may set the new session's title, drawn meanwhile
    let (ws, cx) = open(cx, &s.app);
    let first = focused(&s.app, &ws, cx);
    cx.simulate_keystrokes("cmd-n"); // a second session, now selected
    cx.simulate_keystrokes("cmd-b"); // sidebar hidden: the first session is drawn nowhere
    run_until(&first, r"printf '\033]7501;state=working\007'; echo w-sent; sleep 60", "w-sent");
    let working = |cx: &mut gpui::VisualTestContext| {
        ws.read_with(cx, |ws, _| ws.indicators()).0.iter().any(|i| matches!(i, Some(Indicator::Working { .. })))
    };
    assert!(shows(cx, working), "the app heard the background session is working");
    settle(&focused(&s.app, &ws, cx));
    cx.run_until_parked();
    let before = ws.read_with(cx, |ws, _| ws.signature());
    std::thread::sleep(Duration::from_millis(400));
    cx.run_until_parked();
    let after = ws.read_with(cx, |ws, _| ws.signature());
    assert_eq!(before, after, "the spinner of an off-screen session redraws the window");
    s.app.stop();
}
