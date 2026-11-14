//! A block's cwd as list_blocks reports it (the app's pane header shows it) holds no bidi or
//! control characters, even when the directory's name does.

use std::os::unix::fs::PermissionsExt;

use leon_app::embedded;
use serde_json::json;

#[test]
fn list_blocks_cwd_has_no_bidi_or_control_chars() {
    let dir = tempfile::Builder::new().permissions(std::fs::Permissions::from_mode(0o700)).tempdir().unwrap();
    let evil = dir.path().join("safe\u{202e}txt.sh\u{1b}[31m");
    std::fs::create_dir(&evil).unwrap();
    let app = embedded::start_at(dir.path().join("server.sock")).unwrap();
    let sid = app.call("session.create", json!({})).unwrap()["session_id"].as_str().unwrap().to_string();
    let layout = json!({ "block": { "options": { "command": ["sleep", "60"], "cwd": evil.to_str().unwrap() } } });
    app.call("session.new_window", json!({ "session_id": sid, "layout": layout })).unwrap();
    let blocks = app.call("session.list_blocks", json!({ "session_id": sid })).unwrap();
    let cwds: Vec<String> =
        blocks["blocks"].as_array().unwrap().iter().filter_map(|b| b["cwd"].as_str().map(String::from)).collect();
    app.stop();
    for c in &cwds {
        assert!(
            !c.chars().any(|ch| ch == '\u{202e}' || ch.is_control()),
            "a pane header would show hidden chars from cwd {c:?}"
        );
    }
}
