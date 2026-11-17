//! A popup's program starts at the popup's size across a dropped link
//! (.agent/plans/2026-11-17-popup-size.md): the app's geometry report for a window resized while
//! the link was down fails; once the link is back the app tells the host again, so a popup made
//! there starts at the size the app then gives it.

// macOS only: drives the GPUI app through support/app.rs (the `app` feature).
#![cfg(all(feature = "app", target_os = "macos"))]

#[path = "support/app.rs"]
mod app;
mod support;

use app::*;
use gpui::{px, size, TestAppContext};
use leon_app::hosts::HostState;
use serde_json::json;

#[gpui::test]
fn a_geometry_missed_while_the_link_was_down_is_told_again(cx: &mut TestAppContext) {
    let h = hosted();
    let (sid, b) = session_running(&h.r.remote, "api", "echo hello-remote; exec sleep 300");
    let (ws, cx) = open(cx, &h, vec![host("devbox", "ssh://box")], None);
    wait_connected(&ws, cx, "devbox");
    assert!(shows(cx, |cx| ws.read_with(cx, |ws, _| ws.host_rows()[1].sessions.iter().any(|s| s.id == sid))));
    select(&ws, cx, &sid);
    let shown = |cx: &mut gpui::VisualTestContext| ws.read_with(cx, |ws, _| ws.screen_text(&b)).is_some();
    assert!(shows(cx, shown), "the host's pane shows");

    std::fs::write(h.r.fake.path().join("down"), "").unwrap();
    assert!(h.r.kill_transports() >= 3);
    let reconnecting =
        |cx: &mut gpui::VisualTestContext| matches!(state(&ws, cx, "devbox"), Some(HostState::Reconnecting { .. }));
    assert!(shows(cx, reconnecting), "the link is down");
    // the report of the new size fails: the link is down
    cx.simulate_resize(size(px(1033.), px(777.)));
    cx.run_until_parked();
    std::fs::remove_file(h.r.fake.path().join("down")).unwrap();
    wait_connected(&ws, cx, "devbox");

    let pop = ["-s", &sid, "toggle", "pop", "--create", "--float", "--", "sh", "-c", "stty size; exec sleep 300"];
    h.r.remote.ok(&pop);
    let id =
        h.r.remote.json(&["-s", &sid, "block", "inspect", "pop", "--json"])["block_id"].as_str().unwrap().to_string();
    let first = h.r.remote.screen_until(&id, "stty size", |s| s.lines().any(|l| !l.trim().is_empty()));
    let first = first.lines().find(|l| !l.trim().is_empty()).unwrap().trim().to_string();
    // the app draws it and fits it (a queued call over the link); a marker queued after the fit
    // comes back once the fit has run
    let drew = |cx: &mut gpui::VisualTestContext| {
        ws.read_with(cx, |ws, _| ws.screen_text(&id)).is_some_and(|t| t.contains(first.as_str()))
    };
    let drawn = shows(cx, drew);
    let remote = ws.read_with(cx, |ws, _| ws.host_remote("devbox")).expect("devbox has a remote");
    let (tx, rx) = std::sync::mpsc::channel();
    remote.send_then("server.version", json!({}), move |_| {
        let _ = tx.send(());
    });
    let ran = rx.recv_timeout(std::time::Duration::from_secs(10)).is_ok();
    let call = json!({ "session_id": sid, "block_id": id, "method": "dev.leon.terminal.size", "args": {} });
    let s = h.r.remote.json(&["api", "call", "block.method", &call.to_string()]);
    let now = format!("{} {}", s["rows"], s["cols"]);
    h.local.stop();
    assert!(drawn && ran, "the app drew the popup and its calls ran");
    assert_eq!(first, now, "the popup started at the size the app gives it");
}
