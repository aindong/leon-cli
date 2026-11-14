//! Review fix: `server.stop` decides under the state lock, so no session is created after a stop
//! was accepted (it would be ended by the shutdown that follows), and `if_idle` refuses when
//! sessions exist.

use std::sync::Arc;

use serde_json::json;

#[tokio::test]
async fn no_session_is_created_once_a_stop_was_accepted() {
    let info = leon_server::Info { pid: 1, version: "test", socket: "/nonexistent/server.sock".into() };
    let ctx = Arc::new(leon_server::Ctx::new(info, false));
    let busy =
        ctx.call("session.create", json!({ "layout": { "block": { "options": { "command": ["sleep", "30"] } } } }));
    let sid = busy.await.unwrap()["session_id"].as_str().unwrap().to_string();
    let e = ctx.call("server.stop", json!({ "if_idle": true })).await.unwrap_err();
    assert_eq!(e.code, "conflict");
    ctx.call("session.create", json!({})).await.expect("a refused stop changes nothing");

    ctx.call("session.destroy", json!({ "session_id": sid })).await.unwrap();
    let all: Vec<String> = ctx.state().sessions.iter().map(|s| s.id.clone()).collect();
    for id in all {
        ctx.call("session.destroy", json!({ "session_id": id })).await.unwrap();
    }
    ctx.call("server.stop", json!({ "if_idle": true })).await.expect("idle: the stop is accepted");
    let e = ctx.call("session.create", json!({})).await.unwrap_err();
    assert_eq!(e.code, "shutting_down");
}
