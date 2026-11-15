//! M16b criterion 8: a stalled link never stalls the UI: keys, layout changes and pastes for a
//! remote pane queue off the UI thread and happen once the link moves again; a link that stays
//! stalled is found dead and resumed. Audit A1 (the 0.1.0 app audit): the wheel, scrollback,
//! find, selection, copy, cmd+hover and hint pastes on a remote pane don't wait on the link either,
//! and a local pane answers within 100 ms meanwhile.

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
    panic!("{what} over a stalled link held the window each time: {took:?}");
}

#[gpui::test]
fn a_stalled_link_never_blocks_typing(cx: &mut TestAppContext) {
    let h = hosted();
    let script = "seq 1 300; echo go https://example.com/x; echo ready-remote; exec sh";
    let (sid, b) = session_running(&h.r.remote, "api", script);
    let (ws, cx) = open(cx, &h, vec![host("devbox", "ssh://box")], None);
    wait_connected(&ws, cx, "devbox");
    assert!(shows(cx, |cx| ws.read_with(cx, |ws, _| ws.host_rows()[1].sessions.iter().any(|s| s.id == sid))));
    select(&ws, cx, &sid);
    let ok = shows(cx, |cx| ws.read_with(cx, |ws, _| ws.screen_text(&b).is_some_and(|t| t.contains("ready-remote"))));
    assert!(ok);
    // a first key, so the key path's connection is open before the stall
    cx.simulate_input("e");
    assert!(shows(cx, |_| h.r.remote.capture(&b).contains('e')));

    assert!(signal_transports(&h.r, libc::SIGSTOP) >= 3);
    let t = Instant::now();
    for _ in 0..20 {
        cx.simulate_input("x");
        cx.run_until_parked();
    }
    act(&ws, cx, "pane.split.right", None);
    act(&ws, cx, "client.tab.new", None);
    act(&ws, cx, "client.tab.next", None);
    cx.write_to_clipboard(gpui::ClipboardItem::new_string("pasted".into()));
    act(&ws, cx, "pane.paste", None);
    let took = t.elapsed();

    // the calls whose replies the window uses don't wait on the link either
    let cmd = Modifiers { platform: true, ..Modifiers::none() };
    let at = |cx: &mut gpui::VisualTestContext, row, col| {
        ws.read_with(cx, |ws, _| ws.cell_position(&b, row, col)).expect("the remote pane was painted")
    };
    let (top, mid, link) = (at(cx, 0, 2), at(cx, 3, 10), at(cx, 1, 8));
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
    timed(cx, "back to the bottom", |cx| act(&ws, cx, "client.scroll.bottom", None));
    act(&ws, cx, "client.hints", None); // labelling is local work (and slow in a debug build): not timed
    let labels = ws.read_with(cx, |ws, _| ws.hint_labels(&b));
    let (label, _) = labels.first().expect("the link on screen has a hint").clone();
    timed(cx, "a hint pasted", |cx| cx.simulate_input(&label.to_uppercase()));
    let local = h.local.call("session.list", serde_json::json!({})).unwrap()["sessions"][0]["session_id"]
        .as_str()
        .unwrap()
        .to_string();
    select(&ws, cx, &local);
    let mine = ws.read_with(cx, |ws, _| ws.target_block()).unwrap();
    timed(cx, "keys in a local pane", |cx| cx.simulate_input("echo local-$((5*5))"));
    cx.simulate_keystrokes("enter");
    let ok = shows(cx, |cx| ws.read_with(cx, |ws, _| ws.screen_text(&mine).is_some_and(|t| t.contains("local-25"))));
    assert!(ok, "the local pane answered while the link was stalled");
    select(&ws, cx, &sid);
    signal_transports(&h.r, libc::SIGCONT);
    assert!(took < Duration::from_secs(1), "keys, split, tab and paste over a stalled link took {took:?}");
    let all = "e".to_string() + &"x".repeat(20);
    let ok = shows(cx, |_| h.r.remote.capture(&b).contains(&all));
    assert!(ok, "the keys arrive in order once the link moves: {}", h.r.remote.capture(&b));
    let ok = shows(cx, |_| h.r.remote.view(&sid)["windows"].as_array().unwrap().len() == 2);
    assert!(ok, "the tab was opened on devbox: {}", h.r.remote.view(&sid));
    let layout = h.r.remote.view(&sid)["windows"][0]["layout"].clone();
    assert!(layout.get("split").is_some(), "and the split made: {layout}");

    // stalled for good: the link is found dead and the stream resumed on new transports
    let remote = ws.read_with(cx, |ws, _| ws.host_remote("devbox")).unwrap();
    let resumed = remote.resumes();
    assert!(signal_transports(&h.r, libc::SIGSTOP) >= 3);
    let end = Instant::now() + Duration::from_secs(25);
    while remote.resumes() == resumed && Instant::now() < end {
        cx.run_until_parked();
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(remote.resumes() > resumed, "a half-dead link is noticed and resumed");
    wait_connected(&ws, cx, "devbox");
    select(&ws, cx, &sid);
    let target = ws.read_with(cx, |ws, _| ws.target_block()).unwrap();
    cx.simulate_keystrokes("enter"); // whatever the stall left typed on this pane's line runs first
    cx.simulate_input("echo alive-$((3*4))");
    cx.simulate_keystrokes("enter");
    let ok = shows(cx, |cx| ws.read_with(cx, |ws, _| ws.screen_text(&target).is_some_and(|t| t.contains("alive-12"))));
    assert!(
        ok,
        "typing and screens work on the new transports: {:?}",
        ws.read_with(cx, |ws, _| ws.screen_text(&target))
    );
    h.r.kill_transports();
    h.local.stop();
}
