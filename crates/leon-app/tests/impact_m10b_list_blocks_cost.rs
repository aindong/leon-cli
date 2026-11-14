//! M10b impact: `session.list_blocks` now reports each block's title and cwd (a proc_pidinfo call
//! per block, made under the state lock), and the app refetches it after every batch of events.
//! It must stay cheap with many blocks.

use std::os::unix::fs::PermissionsExt;
use std::time::Instant;

use leon_app::embedded;
use serde_json::json;

#[test]
fn list_blocks_stays_cheap_with_many_blocks() {
    let dir = tempfile::Builder::new().permissions(std::fs::Permissions::from_mode(0o700)).tempdir().unwrap();
    let app = embedded::start_at(dir.path().join("server.sock")).unwrap();
    let sid = app.call("session.create", json!({})).unwrap()["session_id"].as_str().unwrap().to_string();
    let sleeper = json!({ "block": { "options": { "command": ["sleep", "60"] } } });
    for _ in 0..31 {
        app.call("session.new_window", json!({ "session_id": sid, "layout": sleeper })).unwrap();
    }
    let blocks = app.call("session.list_blocks", json!({ "session_id": sid })).unwrap();
    let all = blocks["blocks"].as_array().unwrap();
    assert_eq!(all.len(), 32);
    assert!(all.iter().all(|b| b["cwd"].is_string()), "every live block reports a cwd: {blocks}");
    let n = 100;
    let start = Instant::now();
    for _ in 0..n {
        app.call("session.list_blocks", json!({ "session_id": sid })).unwrap();
    }
    let each = start.elapsed() / n;
    eprintln!("session.list_blocks with 32 blocks: {each:?} per call");
    assert!(each.as_millis() < 5, "list_blocks took {each:?} per call with 32 blocks");
    app.stop();
}
