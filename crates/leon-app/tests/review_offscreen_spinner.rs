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
    let first_id = ws.read_with(cx, |ws, _| ws.target_block()).unwrap();
    let first = term(&s.app, &first_id);
    cx.simulate_keystrokes("cmd-n"); // a second session, now selected
    cx.simulate_keystrokes("cmd-b"); // sidebar hidden: the first session is drawn nowhere
    run_until(&first, r"printf '\033]7501;state=working\007'; echo w-sent; sleep 60", "w-sent");
    let working = |cx: &mut gpui::VisualTestContext| {
        ws.read_with(cx, |ws, _| ws.indicators()).0.iter().any(|i| matches!(i, Some(Indicator::Working { .. })))
    };
    assert!(shows(cx, working), "the app heard the background session is working");
    // each pane's program is reported on the server's 200 ms look, and a tab's kind follows it:
    // wait for `sleep` in front of the first pane, and the new session's shell at its prompt
    let second_id = ws.read_with(cx, |ws, _| ws.target_block()).unwrap();
    let second = term(&s.app, &second_id);
    run_until(&second, "echo ready", "ready");
    let program = |cx: &mut gpui::VisualTestContext, id: &str| {
        let remote = ws.read_with(cx, |ws, _| ws.host_remote(leon_app::hosts::THIS_MAC)).unwrap();
        let model = remote.model();
        model.block(id).and_then(|(_, b)| b.foreground.clone())
    };
    let known = |cx: &mut gpui::VisualTestContext| {
        let shell = second.foreground();
        program(cx, &first_id) == Some(Some("sleep".into()))
            && shell.is_some()
            && program(cx, &second_id) == Some(shell)
    };
    assert!(shows(cx, known), "the app knows each pane's program: {:?}", program(cx, &first_id));
    settle(&second);
    cx.run_until_parked();
    let before = ws.read_with(cx, |ws, _| ws.renders());
    std::thread::sleep(Duration::from_millis(400));
    cx.run_until_parked();
    let after = ws.read_with(cx, |ws, _| ws.renders());
    assert_eq!(before, after, "the spinner of an off-screen session redraws the window");
    s.app.stop();
}
