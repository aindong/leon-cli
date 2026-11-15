//! Two settings written at once (the Settings window lets a second click through while the
//! first write runs): both must land and neither may fail.
use std::sync::{Arc, Barrier};
use std::thread;

use serde_json::{json, Value};

#[test]
fn two_writes_at_once_both_land() {
    for round in 0..20 {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("init.lua");
        std::fs::write(&path, "leon.set(\"font.size\", 13)\n").unwrap();
        let go = Arc::new(Barrier::new(2));
        let spawn = |key: &'static str, v: Value| {
            let (path, go) = (path.clone(), go.clone());
            thread::spawn(move || {
                go.wait();
                leon_lua::rewrite::write_setting(&path, key, &v)
            })
        };
        let a = spawn("copy_on_select", json!(true));
        let b = spawn("notifications.done", json!(false));
        let (a, b) = (a.join().unwrap(), b.join().unwrap());
        assert!(a.is_ok() && b.is_ok(), "round {round}: a write failed: {a:?} {b:?}");
        let s = leon_lua::load(&path).unwrap().settings;
        assert!(s.copy_on_select && !s.notifications.done, "round {round}: a write was lost");
    }
}
