//! M16b criterion 7: a host's group collapses and expands from its label, the choice is kept in
//! app.json for the next launch, and top tab style lists hosts › sessions.

mod support;
#[path = "support/app.rs"]
mod app;

use app::*;
use gpui::{Modifiers, TestAppContext};

fn click(cx: &mut gpui::VisualTestContext, sel: &'static str) {
    assert!(drawn(cx, sel), "{sel} is drawn");
    let b = cx.debug_bounds(sel).unwrap();
    cx.simulate_click(gpui::point(b.origin.x + b.size.width / 2., b.origin.y + b.size.height / 2.), Modifiers::none());
    cx.run_until_parked();
}

fn sessions_drawn(cx: &mut gpui::VisualTestContext) -> usize {
    (0..8).filter(|i| drawn(cx, &format!("session-{i}"))).count()
}

#[gpui::test]
fn hosts_collapse_and_stay_collapsed(cx: &mut TestAppContext) {
    let h = hosted();
    session_running(&h.r.remote, "api", "exec sh");
    let settings = h.dir.path().join("app.json");
    let hosts = vec![host("devbox", "ssh://box")];
    {
        let (ws, cx) = open(cx, &h, hosts.clone(), Some(settings.clone()));
        wait_connected(&ws, cx, "devbox");
        assert!(shows(cx, |cx| sessions_drawn(cx) == 2), "a session on each machine");
        click(cx, "host-1");
        assert!(ws.read_with(cx, |ws, _| ws.host_rows()[1].collapsed));
        assert_eq!(sessions_drawn(cx), 1, "devbox's session is hidden");
        let saved = std::fs::read_to_string(&settings).unwrap();
        assert!(saved.contains("devbox"), "kept in app.json: {saved}");
    }
    let (ws, cx) = open(cx, &h, hosts, Some(settings.clone()));
    wait_connected(&ws, cx, "devbox");
    assert!(ws.read_with(cx, |ws, _| ws.host_rows()[1].collapsed), "still collapsed at the next launch");
    click(cx, "host-1");
    assert!(!ws.read_with(cx, |ws, _| ws.host_rows()[1].collapsed));
    assert!(shows(cx, |cx| sessions_drawn(cx) == 2));

    act(&ws, cx, "client.tab_style.toggle", None);
    assert!(drawn(cx, "host-0") && drawn(cx, "host-1"), "top style keeps the host level");
    assert_eq!(sessions_drawn(cx), 2, "with session rows");
    assert!(!drawn(cx, "row-0-0") && !drawn(cx, "row-1-0"), "and no tab rows: the tabs are in the titlebar");
    h.local.stop();
}
