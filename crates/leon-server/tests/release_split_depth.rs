//! Release audit: a window's splits nest at most 32 deep. Past about 60, the saved state nests
//! deeper than the 128 levels serde_json reads, so the next start moved the whole file aside and
//! every session was lost; `session.view` replies broke the same way for clients.

use std::sync::Arc;

use leon_server::{persist, Ctx, Info};
use serde_json::{json, Value};

const SLEEP: &str = r#"{ "block": { "options": { "command": ["sleep", "30"] } } }"#;

async fn split(c: &Arc<Ctx>, sid: &Value, anchor: &Value) -> Result<Value, leon_core::ApiError> {
    let sleep: Value = serde_json::from_str(SLEEP).unwrap();
    let p =
        json!({ "session_id": sid, "anchor_id": anchor, "direction": "vertical", "side": "after", "layout": sleep });
    c.call("session.new_split", p).await
}

#[tokio::test]
async fn splits_stop_at_a_depth_the_saved_state_can_load() {
    let c = Arc::new(Ctx::new(Info { pid: 1, version: "t", socket: "/nonexistent".into() }, false));
    let sleep: Value = serde_json::from_str(SLEEP).unwrap();
    let r = c.call("session.create", json!({ "layout": sleep })).await.unwrap();
    let sid = r["session_id"].clone();
    let mut newest = r["block_id"].clone();
    let mut made = 0;
    let mut refused = None;
    for _ in 0..70 {
        match split(&c, &sid, &newest).await {
            Ok(r) => {
                newest = r["block_id"].clone();
                made += 1;
            }
            Err(e) => {
                refused = Some(e);
                break;
            }
        }
    }
    // a move can't nest it deeper either: another session's block joining beside the deepest one
    let other = c.call("session.create", json!({ "layout": sleep })).await.unwrap();
    let mv = json!({ "session_id": sid, "block_id": other["block_id"], "anchor_block_id": newest, "direction": "vertical", "side": "after" });
    let moved = c.call("session.move_block", mv).await;

    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("sessions.json");
    persist::save(&c.state(), &file).unwrap();
    let parsed = serde_json::from_slice::<Value>(&std::fs::read(&file).unwrap());
    c.end_programs(std::time::Duration::from_millis(200)).await;

    assert_eq!(made, 32, "made {made} nested splits; the 33rd should be refused");
    assert_eq!(refused.map(|e| e.status), Some(400));
    assert_eq!(moved.map_err(|e| e.status), Err(400), "a move nested the layout past 32");
    assert!(parsed.is_ok(), "the deepest layout the server allows doesn't load: {parsed:?}");
}
