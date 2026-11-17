//! M9 criterion 2: OSC 7501 from a real program shows in the tab, the sidebar row and the pane
//! corner; done clears when the user focuses its pane; the spinner redraws at a capped rate.

mod m9_support;

use std::time::{Duration, Instant};

use gpui::TestAppContext;
use leon_app::status::Indicator;
use leon_core::program_status::State;
use m9_support::*;

// base64 of "Claude" and "thinking". The sleep keeps the shell's next prompt (which may carry an
// OSC 133 A that drops working records) away.
const WORKING: &str =
    r"printf '\033]7501;state=working:progress=40:title=Q2xhdWRl:msg=dGhpbmtpbmc=\007'; echo w-sent; sleep 60";
const DONE: &str = r"printf '\033]7501;state=done\007'; echo d-sent";

#[gpui::test]
fn status_shows_in_tab_sidebar_and_corner_and_done_clears_on_focus(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    cx.simulate_keystrokes("cmd-d");
    let (w, _) = window(&s.app, &ws, cx);
    let [a, b] = <[String; 2]>::try_from(leaves(&w)).unwrap();
    cx.simulate_keystrokes("cmd-alt-left");
    assert_eq!(window(&s.app, &ws, cx).0.focus.as_deref(), Some(a.as_str()));

    run_until(&term(&s.app, &b), DONE, "d-sent"); // b reports done while a has the focus
    run_until(&term(&s.app, &a), WORKING, "w-sent");
    let working = Some(Indicator::Working { progress: Some(40) });
    shows(cx, |cx| ws.read_with(cx, |ws, _| ws.indicators()).1 == vec![working]);
    let (sessions, tabs) = ws.read_with(cx, |ws, _| ws.indicators());
    assert_eq!(tabs, vec![working], "the tab shows its panes' highest state");
    assert_eq!(sessions, vec![working], "so does the session's sidebar row");
    let corner = ws.read_with(cx, |ws, _| ws.pane_caption(&a)).expect("a corner caption");
    assert!(corner.contains("Claude") && corner.contains("thinking"), "{corner:?}");
    assert_eq!(ws.read_with(cx, |ws, _| ws.pane_caption(&b)), None, "done has no title or msg");

    let states = |id: &str| term(&s.app, id).program_status().into_iter().map(|(_, r)| r.state).collect::<Vec<_>>();
    assert_eq!(states(&b), vec![State::Done], "done survives while the user is elsewhere");
    cx.simulate_keystrokes("cmd-alt-right");
    cx.run_until_parked();
    assert!(states(&b).is_empty(), "focusing the pane clears its done");
    assert_eq!(states(&a), vec![State::Working], "working is left alone");
    s.app.stop();
}

#[gpui::test]
fn status_in_a_background_session_or_hidden_tab_redraws_and_shows(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let shown = ws.read_with(cx, |ws, _| ws.session_id()).unwrap();
    // a second session, made from outside (the CLI would), stays unselected
    let other = s.app.call("session.create", serde_json::json!({})).unwrap();
    let other_block = other["block_id"].as_str().unwrap().to_string();
    // and a hidden tab in the selected session
    let tab = s.app.call("session.new_window", serde_json::json!({ "session_id": shown })).unwrap();
    let hidden = tab["block_id"].as_str().unwrap().to_string();
    let first = s.app.ctx().state().session(&shown).unwrap().windows[0].id.clone();
    s.app.call("session.focus_window", serde_json::json!({ "session_id": shown, "window_id": first })).unwrap();
    cx.run_until_parked();
    assert_eq!(ws.read_with(cx, |ws, _| ws.session_id()).as_deref(), Some(shown.as_str()));

    let before = ws.read_with(cx, |ws, _| ws.signature());
    run_until(&term(&s.app, &other_block), DONE, "d-sent");
    let redraws = shows(cx, |cx| ws.read_with(cx, |ws, _| ws.indicators()).0 == vec![None, Some(Indicator::Done)]);
    assert!(redraws && ws.read_with(cx, |ws, _| ws.signature()) != before, "a background session's status redraws");
    let (sessions, _) = ws.read_with(cx, |ws, _| ws.indicators());
    assert_eq!(sessions, vec![None, Some(Indicator::Done)], "its sidebar row shows it");

    // an error in the shown tab keeps the session's row at error, so only the hidden tab's mark moves
    run_until(&focused(&s.app, &ws, cx), r"printf '\033]7501;state=error\007'; echo e-sent", "e-sent");
    shows(cx, |cx| ws.read_with(cx, |ws, _| ws.indicators()).0[0] == Some(Indicator::Error));
    let before = ws.read_with(cx, |ws, _| ws.signature());
    run_until(&term(&s.app, &hidden), DONE, "d-sent");
    let done = shows(cx, |cx| ws.read_with(cx, |ws, _| ws.indicators()).1.get(1) == Some(&Some(Indicator::Done)));
    assert!(done && ws.read_with(cx, |ws, _| ws.signature()) != before, "a hidden tab's status redraws");
    let (sessions, tabs) = ws.read_with(cx, |ws, _| ws.indicators());
    assert_eq!((sessions[0], tabs), (Some(Indicator::Error), vec![Some(Indicator::Error), Some(Indicator::Done)]));
    s.app.stop();
}

#[gpui::test]
fn the_spinner_redraws_at_most_eight_times_a_second(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let t = focused(&s.app, &ws, cx);
    run_until(&t, "echo ready", "ready");
    // the spinner's clock is the Pulse (GPUI phase 3): its ticks over 400 ms of real time, the
    // test clock moved with it
    let ticks = |cx: &mut gpui::VisualTestContext| {
        let before = ws.read_with(cx, |ws, cx| ws.pulse_state(cx).1);
        let end = Instant::now() + Duration::from_millis(400);
        while Instant::now() < end {
            let t = Instant::now();
            std::thread::sleep(Duration::from_millis(5));
            cx.executor().advance_clock(t.elapsed());
            cx.run_until_parked();
        }
        ws.read_with(cx, |ws, cx| ws.pulse_state(cx).1) - before
    };
    settle(&t);
    assert_eq!(ticks(cx), 0, "nothing working: nothing to animate");
    run_until(&t, WORKING, "w-sent");
    settle(&t);
    assert!(shows(cx, |cx| ws.read_with(cx, |ws, cx| ws.pulse_state(cx).0)), "the clock runs");
    let n = ticks(cx);
    assert!((2..=5).contains(&n), "the spinner turns, at most every 125 ms: {n} ticks in 400 ms");
    s.app.stop();
}
