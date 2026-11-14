//! Review proof: --for ends a script that is busy at top level.
use leon_lua::{run_script, Host, ScriptOptions};
use serde_json::{json, Value};
use std::time::{Duration, Instant};
struct NoServer;
impl Host for NoServer {
    fn call(&mut self, _: &str, _: Value, _: Option<Instant>) -> Result<Value, String> {
        Err("no server".into())
    }
}
#[test]
fn for_ends_a_busy_script() {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let opts = ScriptOptions {
            name: "=t".into(),
            dir: ".".into(),
            args: json!({}),
            session_id: None,
            block_id: None,
            run_for: Some(Duration::from_millis(300)),
            fail_fast: false,
        };
        let _ = tx.send(run_script(b"while true do end", Box::new(NoServer), opts));
    });
    let r = rx.recv_timeout(Duration::from_secs(5));
    assert!(matches!(r, Ok(Ok(0))), "--for 300ms did not end a busy script: {r:?}");
}
