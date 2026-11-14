//! M16b criterion 8: a stalled link never stalls the UI: keys, layout changes and pastes for a
//! remote pane queue off the UI thread and happen once the link moves again; a link that stays
//! stalled is found dead and resumed.

#[path = "support/app.rs"]
mod app;
mod support;

use std::time::{Duration, Instant};

use app::*;
use gpui::TestAppContext;

#[gpui::test]
fn a_stalled_link_never_blocks_typing(cx: &mut TestAppContext) {
    let h = hosted();
    let (sid, b) = session_running(&h.r.remote, "api", "echo ready-remote; exec sh");
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
    signal_transports(&h.r, libc::SIGCONT);
    assert!(took < Duration::from_secs(1), "keys, split, tab and paste over a stalled link took {took:?}");
    let all = "e".to_string() + &"x".repeat(20);
    let ok = shows(cx, |_| h.r.remote.capture(&b).contains(&all));
    assert!(ok, "the keys arrive in order once the link moves: {}", h.r.remote.capture(&b));
    let ok = shows(cx, |_| h.r.remote.view(&sid)["windows"].as_array().unwrap().len() == 2);
    assert!(ok, "the tab was opened on devbox: {}", h.r.remote.view(&sid));
    let layout = h.r.remote.view(&sid)["windows"][0]["layout"].clone();
    assert!(layout.get("children").is_some(), "and the split made: {layout}");

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
    cx.simulate_input("echo alive-$((3*4))");
    cx.simulate_keystrokes("enter");
    let ok = shows(cx, |cx| ws.read_with(cx, |ws, _| ws.screen_text(&target).is_some_and(|t| t.contains("alive-12"))));
    assert!(ok, "typing and screens work on the new transports");
    h.r.kill_transports();
    h.local.stop();
}
