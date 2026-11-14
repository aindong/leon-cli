//! M16b criterion 1 (.agent/plans/2026-11-15-m16b-app-hosts.md): the sidebar lists This Mac first,
//! then each host from the hosts list, each with its sessions and tabs; with no hosts there is no
//! host level at all.

#[path = "support/app.rs"]
mod app;
mod support;

use app::*;
use gpui::TestAppContext;
use leon_app::hosts::HostState;
use serde_json::json;

#[gpui::test]
fn the_sidebar_lists_this_mac_then_each_host(cx: &mut TestAppContext) {
    let h = hosted();
    session_running(&h.r.remote, "api", "echo hello-remote; exec sh");
    h.local.call("session.create", json!({ "label": "here" })).unwrap();

    {
        let (ws, cx) = open(cx, &h, vec![], None);
        let rows = ws.read_with(cx, |ws, _| ws.host_rows());
        assert_eq!(rows.len(), 1, "only This Mac: {rows:?}");
        assert!(!drawn(cx, "host-0"), "no host label without hosts (the M10b look)");
        assert!(drawn(cx, "session-0"));
    }

    std::fs::write(h.r.fake.path().join("hold"), "").unwrap();
    let (ws, cx) = open(cx, &h, vec![host("devbox", "ssh://box")], None);
    assert_eq!(state(&ws, cx, "devbox"), Some(HostState::Connecting), "ssh hasn't answered yet");
    assert!(drawn(cx, "host-1") && drawn(cx, "host-dot-1"), "the host shows while connecting");
    std::fs::remove_file(h.r.fake.path().join("hold")).unwrap();
    wait_connected(&ws, cx, "devbox");
    let ok = shows(cx, |cx| ws.read_with(cx, |ws, _| ws.host_rows()[1].sessions.iter().any(|s| s.title == "api")));
    let rows = ws.read_with(cx, |ws, _| ws.host_rows());
    assert!(ok, "devbox lists its session: {rows:?}");
    let labels: Vec<&str> = rows.iter().map(|r| r.label.as_str()).collect();
    assert_eq!(labels, ["This Mac", "devbox"]);
    assert_eq!(rows[0].state, HostState::Connected);
    assert!(rows[0].sessions.iter().any(|s| s.title == "here"), "{rows:?}");
    assert!(!rows[0].sessions.iter().any(|s| s.title == "api"), "api is devbox's, not This Mac's");
    let api = rows[1].sessions.iter().find(|s| s.title == "api").unwrap();
    assert_eq!(api.tabs.len(), 1, "its tab is listed: {api:?}");
    for sel in ["host-0", "host-1", "host-dot-0", "host-dot-1"] {
        assert!(drawn(cx, sel), "{sel} is drawn");
    }
    h.local.stop();
}
