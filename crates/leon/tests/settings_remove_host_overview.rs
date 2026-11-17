//! Removing a host from Settings while the overview of a later host's session is open: the
//! overview follows that host to its new place (found in review).

// macOS only: drives the GPUI app through support/app.rs (the `app` feature).
#![cfg(all(feature = "app", target_os = "macos"))]

#[path = "support/app.rs"]
mod app;
mod support;

use app::*;
use gpui::TestAppContext;

#[gpui::test]
fn removing_an_earlier_host_keeps_the_open_overview(cx: &mut TestAppContext) {
    let h = hosted();
    use std::os::unix::fs::DirBuilderExt;
    std::fs::DirBuilder::new().mode(0o700).create(h.r.local.state_dir()).unwrap();
    let (sid, _) = session_running(&h.r.remote, "api", "exec sh");
    let hosts = vec![host("unreachable-box", "ssh://unreachable-box"), host("devbox", "ssh://box")];
    leon_hosts::hosts::save(&hosts).unwrap();
    let (ws, cx) = open(cx, &h, hosts, None);
    wait_connected(&ws, cx, "devbox");
    assert!(shows(cx, |cx| ws.read_with(cx, |ws, _| ws.host_rows()[2].sessions.iter().any(|s| s.id == sid))));
    select(&ws, cx, &sid);
    act(&ws, cx, "client.overview.session.toggle", None);
    let before = ws.read_with(cx, |ws, _| ws.overview_cards().len());
    assert!(before > 0, "the overview of devbox's session shows its tab");

    // what Settings › Remote Hosts › Remove… runs on yes
    ws.update(cx, |ws, cx| ws.remove_host("unreachable-box", cx)).unwrap();
    cx.run_until_parked();
    let after = ws.read_with(cx, |ws, _| ws.overview_cards());
    h.local.stop();
    assert_eq!(after.len(), before, "devbox's session lost its cards after removing another host: {after:?}");
    assert!(after.iter().all(|c| c.session_id == sid));
}
