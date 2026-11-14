//! Review proof: a huge or infinite exec timeout doesn't panic.
use leon_lua::{CallCtx, Host, Runtime};
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
        "leon.action{ name = 'a', title = 'a', run = function()\n\
        return leon.exec({ 'true' }, { timeout = math.huge }).ok\n\
      end }\n",
    )
    .unwrap();
    let (rt, _) = Runtime::load(&path, Box::new(NoServer)).unwrap();
    let r = catch_unwind(AssertUnwindSafe(|| {
        rt.run_action("a", json!({}), &CallCtx::default(), Instant::now() + Duration::from_secs(5))
    }));
    assert!(r.is_ok(), "run_action panicked");
}
