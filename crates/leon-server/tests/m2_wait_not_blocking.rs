//! `block.wait` must not block a worker thread: on a single-threaded runtime a pending wait still
//! lets other calls run (.agent/plans/2026-11-14-m2-terminal-blocks.md, criterion 12).

use std::sync::mpsc;
use std::time::Duration;

use serde_json::json;

#[test]
fn a_pending_wait_leaves_the_runtime_free() {
    let (done, finished) = mpsc::channel();
    // The runtime lives on its own thread so a blocking wait shows up as a timeout here, not a hang.
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        rt.block_on(async {
            let info = leon_server::Info { pid: 1, version: "test", socket: "/nonexistent/server.sock".into() };
            let ctx = std::sync::Arc::new(leon_server::Ctx::new(info, false));
            let s = ctx.call("session.create", json!({})).await.unwrap();
            let sid = s["session_id"].as_str().unwrap().to_string();
            let layout = json!({ "block": { "options": { "command": ["sleep", "30"] } } });
            let w = ctx.call("session.new_window", json!({ "session_id": sid, "layout": layout })).await.unwrap();
            let b = w["block_id"].as_str().unwrap().to_string();

            let waiting = tokio::spawn(ctx.call("block.wait", json!({ "block_id": b })));
            tokio::task::yield_now().await;
            let list = ctx.call("session.list", json!({})).await.unwrap();
            assert_eq!(list["sessions"].as_array().unwrap().len(), 1);
            assert!(!waiting.is_finished(), "the wait should still be pending");

            let e = ctx.call("block.wait", json!({ "block_id": b, "timeout_ms": 100 })).await.unwrap_err();
            assert_eq!(e.code, "timeout");

            ctx.call("block.close", json!({ "session_id": sid, "block_id": b })).await.unwrap();
            let status = tokio::time::timeout(Duration::from_secs(10), waiting).await.unwrap().unwrap().unwrap();
            assert!(status["exit_code"].is_i64(), "closing the block ends the wait: {status}");
        });
        done.send(()).unwrap();
    });
    finished.recv_timeout(Duration::from_secs(30)).expect("block.wait blocked the runtime (or the test failed)");
}
