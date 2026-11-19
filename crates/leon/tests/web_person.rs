//! leon in the browser, PR 2 (.agent/plans/2026-11-20-web-full-control.md), criterion 4: the page
//! counts as a person for the permission guard (Peer::Remote: it is always another machine), so its
//! keys reach a block held for a permission prompt. That programs inside leon can't is
//! orch_permission_guard's.

mod support;

use std::time::{Duration, Instant};

use serde_json::{json, Value};
use support::localapi::Serving;
use support::web::{self, Ws};

fn call(ws: &mut Ws, id: u64, method: &str, params: Value) -> Value {
    ws.send(&json!({"id": id, "method": method, "params": params}).to_string());
    ws.until(method, |v| v["id"] == id)
}

#[test]
fn the_page_answers_a_permission_prompt_as_a_person() {
    let s = Serving::start("leon.set(\"serve_web\", true)");
    let token = web::token(s.port);
    let mut ws = Ws::open(s.port, Some(&format!("http://127.0.0.1:{}", s.port)), Some(&token)).expect("let in");
    let worker = "echo ready; while IFS= read -r l; do echo \"GOT: $l\"; done";
    let c = call(
        &mut ws,
        1,
        "session.create",
        json!({"layout": {"block": {"options": {"command": ["sh", "-c", worker]}}}}),
    );
    let (sid, b) = (
        c["result"]["session_id"].as_str().unwrap().to_string(),
        c["result"]["block_id"].as_str().unwrap().to_string(),
    );
    s.env.screen_until(&b, "the worker", |t| t.contains("ready"));
    let out = s.cmd().args(["notify", "-b", &b, "--state", "blocked", "--kind", "permission"]).output().unwrap();
    assert!(out.status.success(), "notify: {}", String::from_utf8_lossy(&out.stderr));

    let write =
        json!({"session_id": sid, "block_id": b, "method": "dev.leon.terminal.write", "args": {"data": "approve"}});
    let r = call(&mut ws, 2, "block.method", write);
    assert!(r.get("error").is_none(), "the page's text was refused: {r}");
    let enter = json!({"session_id": sid, "block_id": b, "event": {"type": "key", "key": "enter"}});
    let r = call(&mut ws, 3, "block.event", enter);
    assert!(r.get("error").is_none(), "the page's key was refused: {r}");
    let end = Instant::now() + Duration::from_secs(10);
    while !s.env.capture(&b).contains("GOT: approve") {
        assert!(Instant::now() < end, "the answer never reached the program:\n{}", s.env.capture(&b));
        std::thread::sleep(Duration::from_millis(100));
    }
}
