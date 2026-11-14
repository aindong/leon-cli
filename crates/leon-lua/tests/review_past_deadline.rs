//! Review proof: a job that waited past its deadline doesn't run at all.
use leon_lua::{CallCtx, Host, Runtime};
use serde_json::{json, Value};
use std::time::{Duration, Instant};
struct NoServer;
impl Host for NoServer {
    fn call(&mut self, _: &str, _: Value, _: Option<Instant>) -> Result<Value, String> {
        Err("no server".into())
    }
}
#[test]
fn an_action_past_its_deadline_does_not_run() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("init.lua");
    std::fs::write(
        &path,
        r#"ran = 0
leon.action{ name = "bump", title = "b", run = function() ran = ran + 1; return ran end }
leon.action{ name = "count", title = "c", run = function() return ran end }"#,
    )
    .unwrap();
    let (rt, _) = Runtime::load(&path, Box::new(NoServer)).unwrap();
    let r = rt.run_action("bump", json!({}), &CallCtx::default(), Instant::now() - Duration::from_secs(1));
    assert!(matches!(&r, Err(e) if e.contains("timed out")), "a job past its deadline still ran: {r:?}");
    let n = rt.run_action("count", json!({}), &CallCtx::default(), Instant::now() + Duration::from_secs(5));
    assert_eq!(n, Ok(json!(0)));
}
