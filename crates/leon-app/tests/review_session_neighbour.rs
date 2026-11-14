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
    let (ws, cx) = cx.add_window_view(move |window, cx| Workspace::new(a, window, cx));
    cx.run_until_parked();
    cx.simulate_keystrokes("cmd-n");
    let second = ws.read_with(cx, |ws, _| ws.session_id()).unwrap();
    cx.simulate_keystrokes("cmd-n");
    let third = ws.read_with(cx, |ws, _| ws.session_id()).unwrap();
    assert!(app.call("session.destroy", json!({ "session_id": third })).is_ok());
    cx.simulate_keystrokes("cmd-b"); // any redraw runs settle
    cx.run_until_parked();
    let now = ws.read_with(cx, |ws, _| ws.session_id());
    app.stop();
    assert_eq!(now, Some(second), "BUG: selection jumped to the first session");
}
