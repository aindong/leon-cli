//! Review proof: a __gc finalizer (hooks are off while it runs) can't outrun the deadline.
use leon_lua::{CallCtx, Host, Runtime};
use serde_json::{json, Value};
use std::sync::mpsc;
use std::time::{Duration, Instant};

struct NoServer;
impl Host for NoServer {
    fn call(&mut self, _: &str, _: Value, _: Option<Instant>) -> Result<Value, String> {
        Err("no server".into())
    }
}

#[test]
fn a_finalizer_cannot_outrun_the_deadline() {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("init.lua");
        std::fs::write(
            &path,
            "leon.action{ name = 'a', title = 'a', run = function()\n\
            local p = newproxy(true)\n\
            getmetatable(p).__gc = function() while true do end end\n\
            p = nil\n\
            collectgarbage()\n\
            return 'done'\n\
          end }\n",
        )
        .unwrap();
        let (rt, _) = Runtime::load(&path, Box::new(NoServer)).unwrap();
        let r = rt.run_action("a", json!({}), &CallCtx::default(), Instant::now() + Duration::from_millis(300));
        let _ = tx.send(format!("{r:?}"));
        std::mem::forget(rt);
    });
    assert!(rx.recv_timeout(Duration::from_secs(5)).is_ok(), "still running after the deadline");
}
