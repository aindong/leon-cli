//! Audit A1 (docs/audit-0.1.0-app.md): a host that floods lines that aren't replies, and never
//! replies, holds neither the window nor a local pane: each operation on its pane returns in under
//! 100 ms. Once the flood stops, its pane takes keys again (each call had a deadline).

// macOS only: drives the GPUI app through support/app.rs (the `app` feature).
#![cfg(all(feature = "app", target_os = "macos"))]

#[path = "support/app.rs"]
mod app;
mod support;

use std::time::{Duration, Instant};

use app::*;
use gpui::{Modifiers, MouseButton, MouseDownEvent, MouseUpEvent, ScrollDelta, ScrollWheelEvent, TestAppContext};

/// Run `f` and let the window settle; fail when it held the window for 100 ms or more on each
/// of three tries (a call waiting on the link takes seconds every time; a busy machine's slow
/// frame, now and then).
fn timed(cx: &mut gpui::VisualTestContext, what: &str, mut f: impl FnMut(&mut gpui::VisualTestContext)) {
    let mut took = Vec::new();
    for _ in 0..3 {
        let t = Instant::now();
        f(cx);
        cx.run_until_parked();
        took.push(t.elapsed());
        if took.last().is_some_and(|d| *d < Duration::from_millis(100)) {
            return;
        }
    }
    panic!("{what} over a flooding link held the window each time: {took:?}");
}

#[gpui::test]
fn a_flooding_host_never_holds_the_window(cx: &mut TestAppContext) {
    let h = hosted();
    let script = "seq 1 300; echo go https://example.com/x; echo ready-remote; exec sh";
    let (sid, b) = session_running(&h.r.remote, "api", script);
    let (ws, cx) = open(cx, &h, vec![host("hostile", "ssh://hostile")], None);
    wait_connected(&ws, cx, "hostile");
    assert!(shows(cx, |cx| ws.read_with(cx, |ws, _| ws.host_rows()[1].sessions.iter().any(|s| s.id == sid))));
    select(&ws, cx, &sid);
    let ok = shows(cx, |cx| ws.read_with(cx, |ws, _| ws.screen_text(&b).is_some_and(|t| t.contains("ready-remote"))));
    assert!(ok);
    cx.simulate_input("e");
    assert!(shows(cx, |_| h.r.remote.capture(&b).contains('e')), "keys go through before the flood");
    let at = |cx: &mut gpui::VisualTestContext, row, col| {
        ws.read_with(cx, |ws, _| ws.cell_position(&b, row, col)).expect("the remote pane was painted")
    };
    let (top, mid, link) = (at(cx, 0, 2), at(cx, 3, 10), at(cx, 1, 8));

    let flood = h.r.fake.path().join("flood");
    std::fs::write(&flood, "").unwrap();
    // the flood ends on its own after a while, so a window it holds fails the test, not the run
    let f = flood.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(10));
        let _ = std::fs::remove_file(f);
    });
    std::thread::sleep(Duration::from_millis(200));
    let cmd = Modifiers { platform: true, ..Modifiers::none() };
    timed(cx, "keys", |cx| cx.simulate_input("xyz"));
    timed(cx, "the wheel", |cx| {
        let delta = ScrollDelta::Lines(gpui::point(0., 5.));
        cx.simulate_event(ScrollWheelEvent { position: mid, delta, ..Default::default() });
    });
    timed(cx, "a page up", |cx| act(&ws, cx, "client.scroll.page_up", None));
    timed(cx, "find", |cx| {
        act(&ws, cx, "client.find.open", None);
        cx.simulate_input("12");
    });
    timed(cx, "closing find", |cx| cx.simulate_keystrokes("escape"));
    timed(cx, "a drag", |cx| {
        let left = MouseButton::Left;
        let none = Modifiers::none();
        cx.simulate_event(MouseDownEvent {
            button: left,
            position: top,
            modifiers: none,
            click_count: 1,
            first_mouse: false,
        });
        cx.simulate_mouse_move(mid, left, none);
        cx.simulate_event(MouseUpEvent { button: left, position: mid, modifiers: none, click_count: 1 });
    });
    timed(cx, "a copy", |cx| act(&ws, cx, "pane.copy", None));
    timed(cx, "cmd+hover", |cx| cx.simulate_mouse_move(link, None::<MouseButton>, cmd));
    timed(cx, "a cmd+click", |cx| {
        let left = MouseButton::Left;
        cx.simulate_event(MouseDownEvent {
            button: left,
            position: top,
            modifiers: cmd,
            click_count: 1,
            first_mouse: false,
        });
        cx.simulate_event(MouseUpEvent { button: left, position: top, modifiers: cmd, click_count: 1 });
    });
    timed(cx, "back to the bottom", |cx| act(&ws, cx, "client.scroll.bottom", None));
    act(&ws, cx, "client.hints", None); // labelling is local work (and slow in a debug build): not timed
    if let Some((label, _)) = ws.read_with(cx, |ws, _| ws.hint_labels(&b)).first().cloned() {
        timed(cx, "a hint pasted", |cx| cx.simulate_input(&label.to_uppercase()));
    }
    timed(cx, "a split", |cx| act(&ws, cx, "pane.split.right", None));
    let local = h.local.call("session.list", serde_json::json!({})).unwrap()["sessions"][0]["session_id"]
        .as_str()
        .unwrap()
        .to_string();
    select(&ws, cx, &local);
    let mine = ws.read_with(cx, |ws, _| ws.target_block()).unwrap();
    timed(cx, "keys in a local pane", |cx| cx.simulate_input("echo local-$((5*5))"));
    cx.simulate_keystrokes("enter");
    let ok = shows(cx, |cx| ws.read_with(cx, |ws, _| ws.screen_text(&mine).is_some_and(|t| t.contains("local-25"))));
    assert!(ok, "the local pane answered during the flood");
    let _ = std::fs::remove_file(&flood);

    // the flood is over: the calls it held ran out of time, and new ones go through
    select(&ws, cx, &sid);
    let end = Instant::now() + Duration::from_secs(40);
    let mut typed = false;
    while Instant::now() < end && !typed {
        let target = ws.read_with(cx, |ws, _| ws.target_block()).unwrap();
        cx.simulate_input(" echo after-$((2*3))");
        cx.simulate_keystrokes("enter");
        let t = Instant::now();
        while t.elapsed() < Duration::from_secs(4) && !typed {
            cx.run_until_parked();
            typed = h.r.remote.capture(&target).contains("after-6");
            std::thread::sleep(Duration::from_millis(50));
        }
    }
    assert!(typed, "keys reach the host again once the flood stops");
    h.local.stop();
}
