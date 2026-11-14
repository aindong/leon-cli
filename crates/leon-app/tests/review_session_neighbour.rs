//! Review proof: a session that ends outside a close action hands the selection to its neighbour.

use std::os::unix::fs::PermissionsExt;
use std::rc::Rc;

use gpui::TestAppContext;
use leon_app::embedded;
use leon_app::view::{self, Workspace};
use serde_json::json;

#[gpui::test]
fn an_ended_session_hands_over_to_its_neighbour(cx: &mut TestAppContext) {
    let dir = tempfile::Builder::new().permissions(std::fs::Permissions::from_mode(0o700)).tempdir().unwrap();
    let app = Rc::new(embedded::start_at(dir.path().join("server.sock")).unwrap());
    cx.update(view::init);
    let a = app.clone();
    let (ws, cx) = cx.add_window_view(move |window, cx| {
        Workspace::new(Rc::new(leon_app::remote::Remote::connect(a.socket()).expect("the app attaches")), window, cx)
    });
    cx.run_until_parked();
    cx.simulate_keystrokes("cmd-n");
    let second = ws.read_with(cx, |ws, _| ws.session_id()).unwrap();
    cx.simulate_keystrokes("cmd-n");
    let third = ws.read_with(cx, |ws, _| ws.session_id()).unwrap();
    assert!(app.call("session.destroy", json!({ "session_id": third })).is_ok());
    // the app hears of it by an event a moment later; any redraw runs settle
    let end = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while ws.read_with(cx, |ws, _| ws.session_id()) == Some(third.clone()) && std::time::Instant::now() < end {
        cx.simulate_keystrokes("cmd-b");
        cx.run_until_parked();
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let now = ws.read_with(cx, |ws, _| ws.session_id());
    app.stop();
    assert_eq!(now, Some(second), "BUG: selection jumped to the first session");
}
