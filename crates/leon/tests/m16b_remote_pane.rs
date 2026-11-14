//! M16b criterion 2: a pane on a host renders the host's screen, names the host in its header, and
//! takes typed keys like a local pane.

#[path = "support/app.rs"]
mod app;
mod support;

use app::*;
use gpui::TestAppContext;

#[gpui::test]
fn a_remote_pane_renders_and_takes_input(cx: &mut TestAppContext) {
    let h = hosted();
    let (sid, b) = session_running(&h.r.remote, "api", "echo hello-remote; exec sh");
    let (ws, cx) = open(cx, &h, vec![host("devbox", "ssh://box")], None);
    wait_connected(&ws, cx, "devbox");
    assert!(shows(cx, |cx| ws.read_with(cx, |ws, _| ws.host_rows()[1].sessions.iter().any(|s| s.id == sid))));
    select(&ws, cx, &sid);
    assert_eq!(ws.read_with(cx, |ws, _| ws.target_block()), Some(b.clone()), "the remote pane is focused");

    let ok = shows(cx, |cx| ws.read_with(cx, |ws, _| ws.screen_text(&b).is_some_and(|t| t.contains("hello-remote"))));
    assert!(ok, "the remote screen renders: {:?}", ws.read_with(cx, |ws, _| ws.screen_text(&b)));
    assert!(drawn(cx, &format!("pane-{b}")));
    assert!(drawn(cx, &format!("header-host-{b}")), "the header names the host");
    assert_eq!(ws.read_with(cx, |ws, _| ws.pane_host(&b)), Some("devbox".to_string()));

    cx.simulate_input("echo typed-$((6*7))");
    cx.simulate_keystrokes("enter");
    let ok = shows(cx, |cx| ws.read_with(cx, |ws, _| ws.screen_text(&b).is_some_and(|t| t.contains("typed-42"))));
    assert!(ok, "the keys reached the host and its screen came back");
    assert!(h.r.remote.capture(&b).contains("typed-42"), "the remote block ran it");
    h.local.stop();
}
