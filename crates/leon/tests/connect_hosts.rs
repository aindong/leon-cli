//! The Connect popover's hosts (.agent/plans/2026-11-22-sidebar-footer-connect.md, criterion 4):
//! every host the window shows and every one only in hosts.json, each with its state; choosing a
//! connected one goes to its session, an unreachable one tries again now, one only in the file
//! joins the window and connects.

// macOS only: drives the GPUI app through support/app.rs (the `app` feature).
#![cfg(all(feature = "app", target_os = "macos"))]

#[path = "support/app.rs"]
mod app;
mod support;

use app::*;
use gpui::{Entity, TestAppContext, VisualTestContext};
use leon_app::hosts::HostState;
use leon_app::view::Workspace;
use serde_json::json;

fn hint(ws: &Entity<Workspace>, cx: &mut VisualTestContext, id: &str) -> Option<String> {
    ws.read_with(cx, |ws, cx| {
        let p = ws.connect_menu()?;
        let items = p.read(cx).items().to_vec();
        items.into_iter().find(|i| i.id == id)?.hint
    })
}

fn choose(ws: &Entity<Workspace>, cx: &mut VisualTestContext, id: &str) {
    act(ws, cx, "client.connect", None);
    let sel = format!("popover-item-{id}");
    let p = at(cx, &sel).unwrap_or_else(|| panic!("{sel} is drawn"));
    let mid = gpui::point(p.origin.x + p.size.width / 2., p.origin.y + p.size.height / 2.);
    cx.simulate_click(mid, gpui::Modifiers::none());
    cx.run_until_parked();
}

#[gpui::test]
fn known_hosts_with_their_state(cx: &mut TestAppContext) {
    let h = hosted();
    use std::os::unix::fs::DirBuilderExt;
    std::fs::DirBuilder::new().mode(0o700).create(h.r.local.state_dir()).unwrap();
    let (remote_sid, _) = session_running(&h.r.remote, "api", "exec sh");
    let shown = vec![host("devbox", "ssh://box"), host("unreachable-box", "ssh://unreachable-box")];
    let mut file = shown.clone();
    file.push(host("later", "ssh://box"));
    leon_hosts::hosts::save(&file).unwrap();
    let here = h.local.call("session.create", json!({ "label": "here" })).unwrap();
    let here = here["session_id"].as_str().unwrap().to_string();
    let (ws, cx) = open(cx, &h, shown, None);
    wait_connected(&ws, cx, "devbox");
    let gave_up = shows(cx, |cx| matches!(state(&ws, cx, "unreachable-box"), Some(HostState::Unreachable(_))));
    assert!(gave_up, "{:?}", state(&ws, cx, "unreachable-box"));
    select(&ws, cx, &here);

    act(&ws, cx, "client.connect", None);
    assert_eq!(hint(&ws, cx, "host:devbox").as_deref(), Some("connected"));
    assert_eq!(hint(&ws, cx, "host:unreachable-box").as_deref(), Some("unreachable"));
    assert_eq!(hint(&ws, cx, "host:later").as_deref(), Some("not shown"), "one only in hosts.json");
    let labels: Vec<String> = ws
        .read_with(cx, |ws, cx| ws.connect_menu().unwrap().read(cx).items().iter().map(|i| i.label.clone()).collect());
    assert!(labels.contains(&"Your hosts".to_string()), "a heading: {labels:?}");
    assert!(!labels.contains(&"This Mac".to_string()), "This Mac isn't a host to connect to: {labels:?}");
    cx.simulate_keystrokes("escape");

    // a connected one: go to it
    choose(&ws, cx, "host:devbox");
    assert_eq!(ws.read_with(cx, |ws, _| ws.session_id()), Some(remote_sid.clone()), "its session is selected");

    // an unreachable one: try again now, not at the backoff's end
    let before = ws.read_with(cx, |ws, _| ws.host_attempts(2));
    choose(&ws, cx, "host:unreachable-box");
    let after = ws.read_with(cx, |ws, _| ws.host_attempts(2));
    assert!(after > before, "a new attempt started at once: {before} then {after}");

    // only in the file: joins the window and connects
    choose(&ws, cx, "host:later");
    assert!(ws.read_with(cx, |ws, _| ws.host_rows().iter().any(|r| r.label == "later")), "it is shown");
    wait_connected(&ws, cx, "later");
    h.local.stop();
}
