//! The app's embedded server keeps sessions like `leon server run` does: quitting the app (which
//! stops the embedded server, docs/architecture.md) saves them, and the next start restores them.

use std::os::unix::fs::PermissionsExt;

use leon_app::embedded;
use leon_server::persist::{Persist, FILE};
use serde_json::json;

#[test]
fn quitting_the_app_saves_and_the_next_start_restores() {
    let dir = tempfile::Builder::new().permissions(std::fs::Permissions::from_mode(0o700)).tempdir().unwrap();
    let socket = dir.path().join("server.sock");
    let persist = Some(Persist { file: dir.path().join("state").join(FILE), restore: true });

    let app = embedded::start_with(socket.clone(), persist.clone(), None).expect("embedded server starts");
    let shell = json!({ "block": { "options": { "command": ["sleep", "300"] } } });
    let made = app.call("session.create", json!({ "label": "kept", "layout": shell })).unwrap();
    app.stop(); // what closing the window does
    assert!(app.stopped(), "the embedded server stopped");
    drop(app);

    let app = embedded::start_with(socket, persist, None).expect("embedded server starts again");
    let ls = app.call("session.list", json!({})).unwrap();
    assert_eq!(ls["sessions"][0]["label"], "kept", "restored after the app quit: {ls}");
    assert_eq!(ls["sessions"][0]["session_id"], made["session_id"], "with its ID");
    app.stop();
}
