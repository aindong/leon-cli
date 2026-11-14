//! Review proof: the sandbox refuses Lua bytecode (Lua 5.1 doesn't verify it).
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
fn bytecode_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("init.lua");
    std::fs::write(
        &path,
        "leon.action{ name = 'a', title = 'a', run = function()\n\
        local ok, f = pcall(function() return loadstring(string.dump(function() return 7 end)) end)\n\
        if ok and f then return 'bytecode loaded' end\n\
        return 'refused'\n\
      end }\n",
    )
    .unwrap();
    let (rt, _) = Runtime::load(&path, Box::new(NoServer)).unwrap();
    let r = rt.run_action("a", json!({}), &CallCtx::default(), Instant::now() + Duration::from_secs(5));
    assert_ne!(r, Ok(json!("bytecode loaded")), "bytecode loaded");
}

#[test]
fn text_chunks_and_require_still_load_but_a_binary_sibling_does_not() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("helper.lua"), "return { n = 41 }\n").unwrap();
    std::fs::write(dir.path().join("bin.lua"), b"\x1bLua\x51\x00garbage").unwrap();
    let path = dir.path().join("init.lua");
    std::fs::write(
        &path,
        "leon.action{ name = 'a', title = 'a', run = function()\n\
        local ok, err = pcall(require, 'bin')\n\
        return { n = require('helper').n + loadstring('return 1')(), bin = ok, err = tostring(err) }\n\
      end }\n",
    )
    .unwrap();
    let (rt, _) = Runtime::load(&path, Box::new(NoServer)).unwrap();
    let r = rt.run_action("a", json!({}), &CallCtx::default(), Instant::now() + Duration::from_secs(5)).unwrap();
    assert_eq!(r["n"], 42, "{r}");
    assert_eq!(r["bin"], false, "{r}");
    assert!(r["err"].as_str().unwrap().contains("binary chunks are not allowed"), "{r}");
}
