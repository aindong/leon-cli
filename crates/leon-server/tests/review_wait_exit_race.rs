//! Review: `block.wait` on a running block that closes on exit returns its status, even when the
//! program exits while the wait is being set up (`leon run --wait` of a quick program got
//! `not_found`: the check for a status and the check for the block took the state lock apart).

use std::time::Duration;

use serde_json::json;

#[tokio::test]
async fn a_wait_set_up_as_the_program_exits_gets_its_status() {
    let info = leon_server::Info { pid: 1, version: "test", socket: "/nonexistent/server.sock".into() };
    let ctx = std::sync::Arc::new(leon_server::Ctx::new(info, false));
    let layout = json!({ "block": { "options": { "command": ["sh", "-c", "sleep 0.3; exit 7"] } } });
    let s = ctx.call("session.create", json!({ "layout": layout })).await.unwrap();
    let b = s["block_id"].as_str().unwrap().to_string();
    let wait = ctx.call("block.wait", json!({ "block_id": b, "timeout_ms": 10_000 }));
    let status = tokio::time::timeout(Duration::from_secs(15), wait).await.unwrap().unwrap();
    assert_eq!(status["exit_code"], 7, "{status}");
}
