//! M16b criterion 5: a host that can't be reached says why; one that needs a password says to log
//! in from a terminal; the app's ssh never prompts; everything else keeps working.

#[path = "support/app.rs"]
mod app;
mod support;

use app::*;
use gpui::TestAppContext;
use leon_app::hosts::HostState;
use serde_json::json;

#[gpui::test]
fn unreachable_hosts_say_why(cx: &mut TestAppContext) {
    let h = hosted();
    session_running(&h.r.remote, "api", "exec sh");
    let local = h.local.call("session.create", json!({ "label": "here" })).unwrap();
    let hosts = vec![
        host("devbox", "ssh://box"),
        host("unreachable-box", "ssh://unreachable-box"),
        host("login-box", "ssh://me@login-box"),
    ];
    let (ws, cx) = open(cx, &h, hosts, None);
    wait_connected(&ws, cx, "devbox");
    let ok = shows(cx, |cx| matches!(state(&ws, cx, "unreachable-box"), Some(HostState::Unreachable(_))));
    assert!(ok, "{:?}", state(&ws, cx, "unreachable-box"));
    let rows = ws.read_with(cx, |ws, _| ws.host_rows());
    let note = rows[2].note.clone().unwrap_or_default();
    assert!(note.contains("Connection refused"), "the reason is ssh's: {note:?}");
    assert!(drawn(cx, "host-note-2"), "and it is drawn under the host");

    let ok = shows(cx, |cx| matches!(state(&ws, cx, "login-box"), Some(HostState::NeedsLogin(_))));
    assert!(ok, "{:?}", state(&ws, cx, "login-box"));
    let rows = ws.read_with(cx, |ws, _| ws.host_rows());
    assert_eq!(rows[3].note.as_deref(), Some("needs login: run `leon hosts check login-box` in a terminal"));

    let calls = h.r.ssh_calls();
    assert!(!calls.is_empty());
    for c in &calls {
        assert!(c.iter().any(|a| a == "BatchMode=yes"), "the app's ssh never prompts: {c:?}");
    }

    let sid = local["session_id"].as_str().unwrap();
    select(&ws, cx, sid);
    act(&ws, cx, "pane.split.right", None);
    let st = h.local.ctx().state();
    let s = st.session(sid).unwrap();
    assert_eq!(s.windows[0].layout.as_ref().unwrap().leaves().len(), 2, "This Mac still works");
    drop(st);
    h.local.stop();
}
