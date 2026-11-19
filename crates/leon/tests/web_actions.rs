//! leon in the browser, PR 2 (.agent/plans/2026-11-20-web-full-control.md), criterion 5: the page's
//! session, tab and pane actions are the existing API, run over its WebSocket in the page's order.

mod support;

use serde_json::{json, Value};
use support::localapi::Serving;
use support::web::{self, Ws};

fn call(ws: &mut Ws, id: u64, method: &str, params: Value) -> Value {
    ws.send(&json!({"id": id, "method": method, "params": params}).to_string());
    let r = ws.until(method, |v| v["id"] == id);
    assert!(r.get("error").is_none(), "{method}: {r}");
    r["result"].clone()
}

fn view(ws: &mut Ws, id: u64, s: &str) -> Value {
    call(ws, id, "session.view", json!({"session_id": s}))
}

#[test]
fn the_pages_actions_change_the_view_as_it_expects() {
    let env = Serving::start("leon.set(\"serve_web\", true)");
    let token = web::token(env.port);
    let mut ws = Ws::open(env.port, Some(&format!("http://127.0.0.1:{}", env.port)), Some(&token)).unwrap();

    // + Session
    let s = call(&mut ws, 1, "session.create", json!({}))["session_id"].as_str().unwrap().to_string();
    // + Tab
    let w2 =
        call(&mut ws, 2, "session.new_window", json!({"session_id": s}))["window_id"].as_str().unwrap().to_string();
    let v = view(&mut ws, 3, &s);
    assert_eq!(v["windows"].as_array().unwrap().len(), 2, "{v}");
    // split right in the new tab's pane
    let pane = v["windows"][1]["focused_block_id"].as_str().unwrap().to_string();
    call(
        &mut ws,
        4,
        "session.new_split",
        json!({"session_id": s, "anchor_id": pane, "direction": "horizontal", "side": "after", "layout": {"block": {}}}),
    );
    let v = view(&mut ws, 5, &s);
    assert_eq!(v["windows"][1]["block_ids"].as_array().unwrap().len(), 2, "split: {v}");
    // click the first pane
    call(&mut ws, 6, "session.focus_block", json!({"session_id": s, "block_id": pane}));
    assert_eq!(view(&mut ws, 7, &s)["windows"][1]["focused_block_id"], pane.as_str());
    // × on the pane
    call(&mut ws, 8, "block.close", json!({"session_id": s, "block_id": pane}));
    assert_eq!(view(&mut ws, 9, &s)["windows"][1]["block_ids"].as_array().unwrap().len(), 1);
    // click the first tab, then × on the second
    let w1 = view(&mut ws, 10, &s)["windows"][0]["window_id"].as_str().unwrap().to_string();
    call(&mut ws, 11, "session.focus_window", json!({"session_id": s, "window_id": w1}));
    call(&mut ws, 12, "session.close_window", json!({"session_id": s, "window_id": w2}));
    let v = view(&mut ws, 13, &s);
    assert_eq!(v["windows"].as_array().unwrap().len(), 1, "{v}");
    assert_eq!(v["windows"][0]["active"], true);
    // × on the session
    call(&mut ws, 14, "session.destroy", json!({"session_id": s}));
    let list = call(&mut ws, 15, "session.list", json!({}));
    assert!(!list["sessions"].as_array().unwrap().iter().any(|x| x["session_id"] == s.as_str()), "{list}");
}
