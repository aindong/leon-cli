//! Finding 3 of the performance baseline: the workspace's poll runs when woken or when something
//! timed is due, never on a fixed 8 ms clock. GPUI's test clock only moves when asked, so `run_for`
//! moves it with the real one (the spinner's frames and the find re-run come from the real clock).
mod m20b_support;

use std::time::{Duration, Instant};

use gpui::{Entity, TestAppContext, VisualTestContext};
use leon_app::view::Workspace;
use m20b_support::*;
use serde_json::json;

const WORKING: &str = r"printf '\033]7501;state=working\007'; echo w-sent; sleep 60";
const BLOCKED: &str = r"printf '\033]7501;state=blocked:kind=permission\007'; echo b-sent; sleep 60";

/// Let `d` of real time pass, the test clock with it; returns how many times the poll ran and how
/// many of those asked for a redraw.
fn run_for(ws: &Entity<Workspace>, cx: &mut VisualTestContext, d: Duration) -> (u64, u64) {
    cx.run_until_parked();
    let before = ws.read_with(cx, |ws, _| ws.polls());
    let end = Instant::now() + d;
    while Instant::now() < end {
        let t = Instant::now();
        std::thread::sleep(Duration::from_millis(5));
        cx.executor().advance_clock(t.elapsed());
        cx.run_until_parked();
    }
    let after = ws.read_with(cx, |ws, _| ws.polls());
    (after.0 - before.0, after.1 - before.1)
}

#[gpui::test]
fn an_idle_window_does_not_poll_on_a_clock(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    settle(&focused(&s.app, &ws, cx));
    run_for(&ws, cx, Duration::from_millis(300)); // the prompt's last screen, the first keymap
    let (n, _) = run_for(&ws, cx, Duration::from_secs(2));
    s.app.stop();
    // the old poll ran about 250 times; a late line from the shell can add a wake or two
    assert!(n <= 5, "the poll ran {n} times in 2 s with nothing changing");
}

#[gpui::test]
fn a_working_indicator_ticks_the_pulse_not_the_poll(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    run_until(&focused(&s.app, &ws, cx), WORKING, "w-sent");
    assert!(shows(cx, |cx| ws.read_with(cx, |ws, _| ws.activity_rows().len()) == 1), "the pane reports working");
    run_for(&ws, cx, Duration::from_millis(300));
    let ticks = ws.read_with(cx, |ws, cx| ws.pulse_state(cx).1);
    let (n, _) = run_for(&ws, cx, Duration::from_secs(1));
    let ticks = ws.read_with(cx, |ws, cx| ws.pulse_state(cx).1) - ticks;
    s.app.stop();
    // the spinner's clock is the Pulse's (GPUI phase 3), 8 ticks a second; the poll runs only
    // when woken. Under load a slow step moves the test clock in one jump, and the timer can
    // come round a few times within it.
    assert!(n <= 5, "the poll ran {n} times in 1 s with a working indicator");
    assert!((5..=40).contains(&ticks), "the Pulse ticked {ticks} times in 1 s");
}

#[gpui::test]
fn find_and_activity_waiting_do_not_spin(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let t = focused(&s.app, &ws, cx);
    run_until(&t, BLOCKED, "b-sent");
    assert!(shows(cx, |cx| ws.read_with(cx, |ws, _| ws.dock_badge()).is_some()), "the pane is blocked");
    act(&ws, cx, "client.sidebar.activity.toggle");
    assert!(ws.read_with(cx, |ws, _| ws.activity_visible()));
    cx.simulate_keystrokes("cmd-f b - s e n t");
    assert!(ws.read_with(cx, |ws, _| ws.find_status()).is_some_and(|f| f.1 >= 1), "the find bar has a match");
    run_for(&ws, cx, Duration::from_millis(300));
    let (n, _) = run_for(&ws, cx, Duration::from_secs(2));
    s.app.stop();
    // the waited times tick once a second; the old poll ran about 250 times
    assert!(n <= 8, "the poll ran {n} times in 2 s with find and Activity open and nothing new");
}

#[gpui::test]
fn a_drawer_slides_to_rest_with_no_manual_redraw(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    ws.update(cx, |ws, _| ws.set_reduce_motion(false));
    let sid = sid(&ws, cx);
    let toggle = |create: bool| {
        s.app.call("session.toggle", json!({ "session_id": sid, "label": "scratch", "create": create })).unwrap();
    };
    toggle(true);
    let id = view_window(&s.app, &sid)["docked"][0]["block_id"].as_str().unwrap().to_string();
    let sel = format!("dock-{id}");
    assert!(shows(cx, |cx| drawn(cx, &sel)));
    // its shell's prompt drawn and the first slide over: from here only the poll's clock moves it
    settle(&term(&s.app, &id));
    run_for(&ws, cx, Duration::from_millis(300));
    toggle(false);
    run_for(&ws, cx, Duration::from_millis(300));
    assert!(!drawn(cx, &sel), "gone once it slid out");
    toggle(true);
    assert!(shows(cx, |cx| drawn(cx, &sel)));
    let first = f32::from(at(cx, &sel).unwrap().origin.y);
    run_for(&ws, cx, Duration::from_millis(250));
    let rest = f32::from(at(cx, &sel).unwrap().origin.y);
    let sliding = ws.read_with(cx, |ws, _| ws.sliding());
    s.app.stop();
    assert!(first > rest + 20., "it slid up on its own: first {first}, then {rest}");
    assert!(!sliding, "the slide ended");
}

#[gpui::test]
fn find_runs_again_on_output_that_came_just_after_it(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let t = focused(&s.app, &ws, cx);
    // the typed lines never match: only the output says "zq-"
    run_until(&t, r"printf 'z\161-1\n'", "zq-1");
    cx.simulate_keystrokes("cmd-f z q -");
    let before = ws.read_with(cx, |ws, _| ws.find_status()).map_or(0, |f| f.1);
    assert_eq!(before, 1, "the first line is found");
    // within RERUN (250 ms) of that search: only a timed poll finds it
    t.write(br"printf 'z\161-2\n'".as_slice()).unwrap();
    t.write(b"\r").unwrap();
    assert!(eventually(|| has_line(&t, "zq-2")));
    run_for(&ws, cx, Duration::from_millis(400));
    let after = ws.read_with(cx, |ws, _| ws.find_status()).map_or(0, |f| f.1);
    s.app.stop();
    assert_eq!(after, before + 1, "the find bar counted the new match");
}
