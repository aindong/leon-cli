//! M16b criterion 3: new session, new tab and split act on the focused pane's host; the palette
//! offers New Session on each machine and Add Host.

// macOS only: drives the GPUI app through support/app.rs (the `app` feature).
#![cfg(all(feature = "app", target_os = "macos"))]

#[path = "support/app.rs"]
mod app;
mod support;

use app::*;
use gpui::TestAppContext;
use serde_json::json;

fn leaves(v: &serde_json::Value) -> usize {
    match v.get("split") {
        Some(s) => leaves(&s["before"]) + leaves(&s["after"]),
        None => 1,
    }
}

#[gpui::test]
fn actions_go_to_the_focused_panes_host(cx: &mut TestAppContext) {
    let h = hosted();
    let (sid, _) = session_running(&h.r.remote, "api", "exec sh");
    let local = h.local.call("session.create", json!({ "label": "here" })).unwrap();
    let local_sid = local["session_id"].as_str().unwrap().to_string();
    let (ws, cx) = open(cx, &h, vec![host("devbox", "ssh://box")], None);
    wait_connected(&ws, cx, "devbox");
    assert!(shows(cx, |cx| ws.read_with(cx, |ws, _| ws.host_rows()[1].sessions.iter().any(|s| s.id == sid))));
    select(&ws, cx, &sid);
    let local_windows = || h.local.ctx().state().sessions.iter().map(|s| s.windows.len()).sum::<usize>();
    let local_sessions = h.local.ctx().state().sessions.len();
    let windows_before = local_windows();

    // over a link the window doesn't wait for a change: it lands a moment later
    act(&ws, cx, "pane.split.right", None);
    let ok = shows(cx, |_| leaves(&h.r.remote.view(&sid)["windows"][0]["layout"]) == 2);
    assert!(ok, "the split is on devbox: {}", h.r.remote.view(&sid));
    act(&ws, cx, "client.tab.new", None);
    let ok = shows(cx, |_| h.r.remote.view(&sid)["windows"].as_array().unwrap().len() == 2);
    assert!(ok, "the tab is on devbox");
    act(&ws, cx, "session.new", None);
    assert!(shows(cx, |_| session_ids(&h.r.remote).len() == 2), "the session is on devbox");
    let remote_ids = session_ids(&h.r.remote);
    let ok = shows(cx, |cx| ws.read_with(cx, |ws, _| ws.session_id()).is_some_and(|s| remote_ids.contains(&s)));
    assert!(ok, "and selected");
    assert_eq!(h.local.ctx().state().sessions.len(), local_sessions, "nothing new on This Mac");
    assert_eq!(local_windows(), windows_before);

    let titles: Vec<String> = ws.read_with(cx, |ws, _| ws.palette_rows_for("")).into_iter().map(|(t, _)| t).collect();
    for want in ["New Session on This Mac", "New Session on devbox", "Add Host…"] {
        assert!(titles.iter().any(|t| t == want), "the palette offers {want}: {titles:?}");
    }

    select(&ws, cx, &local_sid);
    act(&ws, cx, "session.new", Some(json!({ "host": "devbox" })));
    assert!(shows(cx, |_| session_ids(&h.r.remote).len() == 3), "New Session on devbox, from This Mac");
    let remote_ids = session_ids(&h.r.remote);
    let ok = shows(cx, |cx| ws.read_with(cx, |ws, _| ws.session_id()).is_some_and(|s| remote_ids.contains(&s)));
    assert!(ok, "the new session is selected");
    act(&ws, cx, "session.new", Some(json!({ "host": "This Mac" })));
    assert_eq!(h.local.ctx().state().sessions.len(), local_sessions + 1, "New Session on This Mac");
    h.local.stop();
}
