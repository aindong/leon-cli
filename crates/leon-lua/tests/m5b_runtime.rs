//! M5b criterion 10 (.agent/plans/2026-11-14-m5b-lua-runtime.md): a config action cannot outrun
//! its deadline, not even by catching the timeout, and a config load is bounded too.

use std::time::{Duration, Instant};

use leon_lua::{CallCtx, Host, Runtime};
use serde_json::{json, Value};

struct NoServer;

impl Host for NoServer {
    fn call(&mut self, _: &str, _: Value, _: Option<Instant>) -> Result<Value, String> {
        Err("no server".into())
    }
}

fn load(src: &str) -> (tempfile::TempDir, Result<Runtime, String>) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("init.lua");
    std::fs::write(&path, src).unwrap();
    let rt = Runtime::load(&path, Box::new(NoServer)).map(|(rt, _cfg)| rt).map_err(|e| e.to_string());
    (dir, rt)
}

const SPINNERS: &str = r#"
leon.action{ name = "pcall_spin", title = "p", run = function()
  while true do pcall(function() while true do end end) end
end }
leon.action{ name = "xpcall_spin", title = "x", run = function()
  while true do xpcall(function() while true do end end, function(e) return e end) end
end }
leon.action{ name = "coroutine_spin", title = "c", run = function()
  while true do coroutine.resume(coroutine.create(function() while true do end end)) end
end }
leon.action{ name = "slow_exec", title = "e", run = function()
  return leon.exec({ "sleep", "5" })
end }
leon.action{ name = "fine", title = "f", run = function() return "still here" end }
"#;

#[test]
fn a_spinning_action_times_out_even_when_it_catches_errors() {
    let (_dir, rt) = load(SPINNERS);
    let rt = rt.unwrap();
    for name in ["pcall_spin", "xpcall_spin", "coroutine_spin"] {
        let t = Instant::now();
        let err = rt.run_action(name, json!({}), &CallCtx::default(), t + Duration::from_millis(300)).unwrap_err();
        assert!(err.contains("timed out"), "{name}: {err}");
        assert!(t.elapsed() < Duration::from_secs(2), "{name} took {:?}", t.elapsed());
        let ok = rt.run_action("fine", json!({}), &CallCtx::default(), Instant::now() + Duration::from_secs(5));
        assert_eq!(ok, Ok(json!("still here")), "after {name}");
    }
}

#[test]
fn exec_in_an_action_gets_only_the_time_left() {
    let (_dir, rt) = load(SPINNERS);
    let rt = rt.unwrap();
    let t = Instant::now();
    let r = rt.run_action("slow_exec", json!({}), &CallCtx::default(), t + Duration::from_millis(300));
    assert!(t.elapsed() < Duration::from_secs(2), "took {:?}", t.elapsed());
    match r {
        Ok(v) => assert_eq!(v["timed_out"], true, "{v}"),
        Err(e) => assert!(e.contains("timed out"), "{e}"),
    }
}

#[test]
fn a_spinning_config_load_fails() {
    let t = Instant::now();
    let (_dir, rt) = load("while true do end\n");
    let err = rt.err().expect("the load must fail");
    assert!(err.contains("timed out"), "{err}");
    assert!(t.elapsed() < Duration::from_secs(8), "took {:?}", t.elapsed());
}
