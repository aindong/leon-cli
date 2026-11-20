//! The web endpoint (.agent/plans/2026-11-19-web-client-tailnet.md), criterion 9: the WebSocket
//! carries the socket's API, one line per message: replies, events and screens.

mod support;

use serde_json::json;
use support::localapi::Serving;
use support::web::{self, Ws};

#[test]
fn the_socket_api_round_trips_over_the_websocket() {
    let s = Serving::start("leon.set(\"serve_web\", true)");
    let token = web::token(s.port);
    let mut ws = Ws::open(s.port, Some(&format!("http://127.0.0.1:{}", s.port)), Some(&token)).expect("let in");

    ws.send(&json!({"id": 1, "method": "server.version"}).to_string());
    let v = ws.until("server.version's reply", |v| v["id"] == 1);
    assert!(v["result"]["api_version"].is_string(), "{v}");

    ws.send(&json!({"id": 2, "method": "session.attach_all", "params": {"event_scope": "all"}}).to_string());
    let a = ws.until("the attach reply", |v| v["id"] == 2);
    assert!(a["result"].is_object(), "{a}");

    ws.send(&json!({"id": 3, "method": "session.create", "params": {"label": "web"}}).to_string());
    let c = ws.until("session.create's reply", |v| v["id"] == 3);
    let session = c["result"]["session_id"].as_str().unwrap_or_else(|| panic!("{c}")).to_string();
    let ev = ws.until("an event", |v| v.get("seq").is_some());
    assert!(ev["event"].is_object(), "{ev}");

    ws.send(&json!({"id": 4, "method": "session.list_blocks", "params": {"session_id": session}}).to_string());
    let b = ws.until("the blocks", |v| v["id"] == 4);
    let block = b["result"]["blocks"][0]["block_id"].as_str().unwrap_or_else(|| panic!("{b}")).to_string();

    ws.send(&json!({"id": 5, "method": "block.watch", "params": {"block_ids": [block]}}).to_string());
    let sc = ws.until("the block's screen", |v| v["screen"]["block_id"] == block.as_str());
    assert_eq!(sc["screen"]["full"], true, "the first screen is whole: {sc}");

    ws.send("not json");
    let e = ws.until("the error for a bad line", |v| v.get("error").is_some());
    assert_eq!(e["id"], serde_json::Value::Null, "{e}");
    ws.send(&json!({"id": 6, "method": "server.status"}).to_string());
    assert_eq!(ws.until("still served", |v| v["id"] == 6)["result"]["state"], "running");
}
