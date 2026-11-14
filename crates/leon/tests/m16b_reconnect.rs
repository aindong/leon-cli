//! M16b criterion 4: a dropped link shows reconnecting with its backoff, keeps the host's panes on
//! screen under a stale overlay, and comes back with the event stream resumed: no event lost.

#[path = "support/app.rs"]
mod app;
mod support;

use app::*;
use gpui::TestAppContext;
use leon_app::hosts::HostState;

#[gpui::test]
fn a_dropped_link_reconnects_and_resumes(cx: &mut TestAppContext) {
    let h = hosted();
    let (sid, b) = session_running(&h.r.remote, "api", "echo hello-remote; exec sh");
    let (ws, cx) = open(cx, &h, vec![host("devbox", "ssh://box")], None);
    wait_connected(&ws, cx, "devbox");
    assert!(shows(cx, |cx| ws.read_with(cx, |ws, _| ws.host_rows()[1].sessions.iter().any(|s| s.id == sid))));
    select(&ws, cx, &sid);
    let shown = |cx: &mut gpui::VisualTestContext, what: &str| {
        ws.read_with(cx, |ws, _| ws.screen_text(&b).is_some_and(|t| t.contains(what)))
    };
    assert!(shows(cx, |cx| shown(cx, "hello-remote")));
    let remote = ws.read_with(cx, |ws, _| ws.host_remote("devbox")).expect("devbox has a remote");
    let seq = remote.seq();

    std::fs::write(h.r.fake.path().join("down"), "").unwrap();
    assert!(h.r.kill_transports() >= 3);
    let reconnecting = |cx: &mut gpui::VisualTestContext| matches!(state(&ws, cx, "devbox"), Some(HostState::Reconnecting { attempt, .. }) if attempt >= 2);
    assert!(shows(cx, reconnecting), "reconnecting, and retrying: {:?}", state(&ws, cx, "devbox"));
    let Some(HostState::Reconnecting { retry_in, .. }) = state(&ws, cx, "devbox") else { unreachable!() };
    assert!(retry_in > std::time::Duration::ZERO, "it says when it tries next");
    assert!(drawn(cx, "stale-overlay"), "the host's panes are marked stale");
    assert!(drawn(cx, &format!("pane-{b}")), "and stay on screen");
    assert!(shown(cx, "hello-remote"), "with what they showed");

    // the host carries on meanwhile; the app must hear about it once back
    h.r.remote.ok(&["new", "while-down"]);
    std::fs::remove_file(h.r.fake.path().join("down")).unwrap();
    wait_connected(&ws, cx, "devbox");
    let ok =
        shows(cx, |cx| ws.read_with(cx, |ws, _| ws.host_rows()[1].sessions.iter().any(|s| s.title == "while-down")));
    assert!(ok, "the session made while the link was down shows up");
    assert!(!drawn(cx, "stale-overlay"));
    assert!(remote.seq() > seq, "the stream went on");
    assert_eq!(remote.gaps(), 0, "every seq came once, in order");
    assert!(remote.resumes() >= 1, "the stream was resumed");
    assert_eq!(remote.reattaches(), 0, "not attached afresh");

    cx.simulate_input("echo back-$((7*8))");
    cx.simulate_keystrokes("enter");
    assert!(shows(cx, |cx| shown(cx, "back-56")), "keys and screens work again");
    h.local.stop();
}
