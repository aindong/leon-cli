//! `server.status` says when the server started and how many blocks it holds (the Settings
//! window's Server page shows both), in API 1.11.0.

use std::sync::Arc;

use leon_server::{Ctx, Info};
use serde_json::{json, Value};

const SLEEP: &str = r#"{ "block": { "options": { "command": ["sleep", "30"] } } }"#;

#[tokio::test]
async fn status_reports_its_start_and_its_blocks() {
    let before = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
    let c = Arc::new(Ctx::new(Info { pid: 1, version: "t", socket: "/nonexistent".into() }, false));
    let s = c.call("server.status", json!({})).await.unwrap();
    assert_eq!(s["blocks"], 0);
    let started = s["started_at"].as_u64().expect("started_at");
    assert!(started >= before && started <= before + 60, "{started} vs {before}");

    let sleep: Value = serde_json::from_str(SLEEP).unwrap();
    let r = c.call("session.create", json!({ "layout": sleep.clone() })).await.unwrap();
    let p = json!({ "session_id": r["session_id"], "anchor_id": r["block_id"], "direction": "vertical", "side": "after", "layout": sleep });
    c.call("session.new_split", p).await.unwrap();
    c.call("session.create", json!({ "layout": sleep })).await.unwrap();
    let s = c.call("server.status", json!({})).await.unwrap();
    assert_eq!((s["sessions"].clone(), s["blocks"].clone()), (json!(2), json!(3)), "{s}");
    assert_eq!(s["started_at"].as_u64(), Some(started), "the start doesn't move");
    let v = c.call("server.version", json!({})).await.unwrap();
    // 1.11.0 brought these fields; a later minor keeps them
    let minor: u64 =
        v["api_version"].as_str().unwrap().strip_prefix("1.").unwrap().split('.').next().unwrap().parse().unwrap();
    assert!(minor >= 11, "{v}");
}
