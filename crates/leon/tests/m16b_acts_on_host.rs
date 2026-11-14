//! M16b criterion 3: new session, new tab and split act on the focused pane's host; the palette
//! offers New Session on each machine and Add Host.

mod support;
#[path = "support/app.rs"]
mod app;

use app::*;
use gpui::TestAppContext;
use serde_json::json;

fn leaves(v: &serde_json::Value) -> usize {
    match v.get("children") {
        Some(c) => c.as_array().unwrap().iter().map(leaves).sum(),
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
    let local_blocks = || h.local.ctx().state().sessions.iter().map(|s| s.windows.len()).sum::<usize>();
    let (local_sessions, local_windows) = (h.local.ctx().state().sessions.len(), local_blocks());

    act(&ws, cx, "pane.split.right", None);
    let view = h.r.remote.view(&sid);
    assert_eq!(leaves(&view["windows"][0]["layout"]), 2, "the split is on devbox: {view}");
    act(&ws, cx, "client.tab.new", None);
    assert_eq!(h.r.remote.view(&sid)["windows"].as_array().unwrap().len(), 2, "the tab is on devbox");
    act(&ws, cx, "session.new", None);
    let remote_ids = session_ids(&h.r.remote);
    assert_eq!(remote_ids.len(), 2, "the session is on devbox");
    let selected = ws.read_with(cx, |ws, _| ws.session_id()).unwrap();
    assert!(remote_ids.contains(&selected), "and selected");
    assert_eq!(h.local.ctx().state().sessions.len(), local_sessions, "nothing new on This Mac");
    assert_eq!(local_blocks(), local_windows);

    let titles: Vec<String> = ws.read_with(cx, |ws, _| ws.palette_rows_for("")).into_iter().map(|(t, _)| t).collect();
    for want in ["New Session on This Mac", "New Session on devbox", "Add Host…"] {
        assert!(titles.iter().any(|t| t == want), "the palette offers {want}: {titles:?}");
    }

    select(&ws, cx, &local_sid);
    act(&ws, cx, "session.new", Some(json!({ "host": "devbox" })));
    let remote_ids = session_ids(&h.r.remote);
    assert_eq!(remote_ids.len(), 3, "New Session on devbox, from This Mac");
    let selected = ws.read_with(cx, |ws, _| ws.session_id()).unwrap();
    assert!(remote_ids.contains(&selected), "the new session is selected");
    act(&ws, cx, "session.new", Some(json!({ "host": "This Mac" })));
    assert_eq!(h.local.ctx().state().sessions.len(), local_sessions + 1, "New Session on This Mac");
    h.local.stop();
}
