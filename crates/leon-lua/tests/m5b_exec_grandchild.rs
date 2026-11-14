//! leon.exec must honour its deadline even when the program has a child holding its stdout.

use std::time::{Duration, Instant};

use leon_lua::{CallCtx, Host, Runtime};
use serde_json::{json, Value};

struct NoServer;

impl Host for NoServer {
    fn call(&mut self, _: &str, _: Value, _: Option<Instant>) -> Result<Value, String> {
        Err("no server".into())
    }
}

#[test]
fn exec_through_a_shell_still_gets_only_the_time_left() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("init.lua");
    std::fs::write(
        &path,
        r#"leon.action{ name = "slow_sh", title = "s", run = function()
  return leon.exec({ "sh", "-c", "sleep 6; true" })
end }
"#,
    )
    .unwrap();
    let (rt, _cfg) = Runtime::load(&path, Box::new(NoServer)).map_err(|e| e.to_string()).unwrap();
    let t = Instant::now();
    let r = rt.run_action("slow_sh", json!({}), &CallCtx::default(), t + Duration::from_millis(300));
    assert!(t.elapsed() < Duration::from_secs(3), "exec outlived its deadline: took {:?}", t.elapsed());
    match r {
        Ok(v) => assert_eq!(v["timed_out"], true, "{v}"),
        Err(e) => assert!(e.contains("timed out"), "{e}"),
    }
}
