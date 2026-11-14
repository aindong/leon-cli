//! Review proof: huge or infinite timeouts and delays don't panic.
use leon_lua::{run_script, CallCtx, Host, Runtime, ScriptOptions};
use serde_json::{json, Value};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::time::{Duration, Instant};
struct NoServer;
impl Host for NoServer {
    fn call(&mut self, _: &str, _: Value, _: Option<Instant>) -> Result<Value, String> {
        Err("no server".into())
    }
}
#[test]
fn a_huge_exec_timeout_does_not_panic() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("init.lua");
    std::fs::write(
        &path,
        r#"leon.action{ name = "huge", title = "h", run = function()
  return leon.exec({ "true" }, { timeout = math.huge }).code
end }"#,
    )
    .unwrap();
    let (rt, _) = Runtime::load(&path, Box::new(NoServer)).unwrap();
    let r = catch_unwind(AssertUnwindSafe(|| {
        rt.run_action("huge", json!({}), &CallCtx::default(), Instant::now() + Duration::from_secs(5))
    }));
    assert!(matches!(&r, Ok(Ok(v)) if *v == json!(0)), "huge exec timeout panicked or failed");
}
#[test]
fn a_huge_timer_delay_does_not_panic() {
    let opts = ScriptOptions {
        name: "=t".into(),
        dir: ".".into(),
        args: json!({}),
        session_id: None,
        block_id: None,
        run_for: None,
        fail_fast: false,
    };
    let r = catch_unwind(AssertUnwindSafe(|| {
        run_script(b"local t = leon.after(math.huge, function() end) t:cancel()", Box::new(NoServer), opts)
    }));
    assert!(matches!(r, Ok(Ok(0))), "huge timer delay panicked or failed");
}
