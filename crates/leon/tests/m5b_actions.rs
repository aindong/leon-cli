//! M5b criteria 1–4, 8, 11, 13 (.agent/plans/2026-11-14-m5b-lua-runtime.md): config actions, the
//! keymap and reloads, through the real binary and a server started with `LEON_CONFIG`.

mod support;

use std::path::PathBuf;
use std::process::{Child, Stdio};
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use support::Env;

const CONFIG: &str = r#"leon.mode("resize", { exclusive = true })
leon.bind("resize/left", "split_here")
leon.action{ name = "split_here", title = "Split here", run = function(ctx, args)
  local r, err = leon.session.new_split{ anchor_id = ctx.block_id, direction = "horizontal", side = "after",
    layout = leon.layout.block{ options = { command = { "sleep", "300" } } } }
  if not r then error(err, 0) end
  return r
end }
leon.bind("ctrl+a", "split_here")
leon.bind("ctrl+x", function(ctx, ev)
  leon.session.set_label{ label = "bound" }
  return true
end)
leon.action{ name = "other", title = "Other", run = function() return "from other" end }
leon.action{ name = "echo_args", title = "Echo", run = function(ctx, args)
  return { args = args, ctx = ctx, nested = leon.invoke("other") }
end }
leon.action{ name = "recurse", title = "Recurse", run = function()
  local r, err = leon.invoke("recurse")
  if not r then error(err, 0) end
  return r
end }
leon.action{ name = "fail_here", title = "Fail", run = function() error("disk full", 0) end }
leon.action{ name = "spin", title = "Spin", run = function(ctx)
  leon.session.set_label{ label = "spinning" }
  while true do end
end }
leon.bind("ctrl+b>c", "other")
"#;

/// A server started with `LEON_CONFIG` at `dir/init.lua` holding `src`.
fn server(src: &str) -> (Env, tempfile::TempDir, PathBuf) {
    let env = Env::new();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("init.lua");
    std::fs::write(&path, src).unwrap();
    let out = env.cmd().env("LEON_CONFIG", &path).args(["server", "start"]).output().unwrap();
    assert!(out.status.success(), "server start: {}", String::from_utf8_lossy(&out.stderr));
    (env, dir, path)
}

fn stderr(env: &Env, args: &[&str]) -> (i32, String) {
    let o = env.run(args);
    (o.status.code().unwrap(), String::from_utf8_lossy(&o.stderr).into_owned())
}

fn binding<'a>(km: &'a Value, key: &str) -> Option<&'a Value> {
    km["bindings"].as_array().unwrap().iter().find(|b| b["key"] == key && b["mode"].is_null())
}

fn blocks_in_first_window(env: &Env, s: &str) -> usize {
    env.window(s, 0)["block_ids"].as_array().unwrap().len()
}

#[test]
fn c1_server_loads_config_and_lists_actions_and_keymap() {
    let (env, _dir, path) = server(CONFIG);
    let actions = env.ok(&["actions"]);
    let line = actions.lines().find(|l| l.contains("split_here")).unwrap_or_else(|| panic!("{actions}"));
    assert!(line.contains("Split here"), "{actions}");

    let km = env.json(&["keymap", "--json"]);
    assert_eq!(km["generation"], 1, "{km}");
    let a = binding(&km, "ctrl+a").unwrap_or_else(|| panic!("no ctrl+a: {km}"));
    assert_eq!(a["action"], "split_here");
    let src = a["source"].as_str().unwrap();
    assert!(src.starts_with(&format!("{}:", path.display())) && src.ends_with(":9"), "{src}");
    assert_eq!(binding(&km, "ctrl+x").unwrap_or_else(|| panic!("no ctrl+x: {km}"))["fn"], true);
    assert!(binding(&km, "ctrl+b>c").is_some(), "{km}");
    let modes = km["modes"].as_array().unwrap();
    assert!(modes.iter().any(|m| m["name"] == "resize" && m["exclusive"] == true), "{km}");
    let mode_bind = km["bindings"].as_array().unwrap().iter().find(|b| b["mode"] == "resize");
    assert_eq!(mode_bind.unwrap_or_else(|| panic!("{km}"))["key"], "left");
    let plain = env.ok(&["keymap"]);
    assert!(plain.lines().any(|l| l.contains("ctrl+a") && l.contains("split_here")), "{plain}");
}

#[test]
fn c2_an_action_splits_a_pane_via_do_and_the_api() {
    let (env, _dir, _) = server(CONFIG);
    let (s, b) = env.session();
    let r = env.json(&["do", "-s", &s, "-b", &b, "split_here"]);
    let new = r["block_id"].as_str().unwrap_or_else(|| panic!("no block_id in {r}"));
    assert_ne!(new, b);
    assert_eq!(blocks_in_first_window(&env, &s), 2);

    let params = json!({ "action": "split_here", "session_id": s, "block_id": b }).to_string();
    let r = env.json(&["api", "call", "client.action", &params]);
    assert!(r["result"]["block_id"].is_string(), "{r}");
    assert_eq!(blocks_in_first_window(&env, &s), 3);
}

#[test]
fn c3_args_origin_and_nested_invoke() {
    let (env, _dir, _) = server(CONFIG);
    let (s, b) = env.session();
    let r = env.json(&["do", "-s", &s, "-b", &b, "echo_args", "n=3", "s=hi", "--args", r#"{"j":true}"#]);
    assert_eq!(r["args"], json!({ "n": 3, "s": "hi", "j": true }), "{r}");
    assert_eq!(r["ctx"]["origin"], "cli", "{r}");
    assert_eq!(r["ctx"]["session_id"], s.as_str(), "{r}");
    assert_eq!(r["ctx"]["block_id"], b.as_str(), "{r}");
    assert_eq!(r["nested"], "from other");

    let (code, err) = stderr(&env, &["do", "recurse"]);
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("16"), "the nesting limit is named: {err}");
}

#[test]
fn c4_an_action_error_exits_1_with_its_message() {
    let (env, _dir, _) = server(CONFIG);
    let (code, err) = stderr(&env, &["do", "fail_here"]);
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("disk full"), "{err}");
    assert!(!err.contains("init.lua:"), "error(msg, 0) has no position: {err}");

    let (code, err) = stderr(&env, &["do", "no_such"]);
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("no_such"), "{err}");
}

#[test]
fn c8_a_broken_reload_keeps_the_last_good_keymap() {
    let (env, _dir, path) = server("leon.bind(\"ctrl+a\", \"foo\")\n");
    let before = env.json(&["keymap", "--json"]);
    assert_eq!(before["generation"], 1, "{before}");
    assert!(binding(&before, "ctrl+a").is_some(), "{before}");
    let mut events = env.events(&["--all"]);

    std::fs::write(&path, "leon.bind(\"ctrl+b\", \"foo\")\nleon.bind(\n").unwrap();
    let (code, err) = stderr(&env, &["config", "reload"]);
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("init.lua:"), "the syntax error has its position: {err}");
    assert_eq!(env.json(&["keymap", "--json"]), before, "a failed reload changes nothing");

    std::fs::write(&path, "leon.bind(\"ctrl+b\", \"foo\")\n").unwrap();
    let (code, err) = stderr(&env, &["config", "reload"]);
    assert_eq!(code, 0, "{err}");
    let after = env.json(&["keymap", "--json"]);
    assert_eq!(after["generation"], 2, "{after}");
    assert!(binding(&after, "ctrl+b").is_some() && binding(&after, "ctrl+a").is_none(), "{after}");
    let e = events.until("keymap_changed");
    assert_eq!(e.payload["generation"], 2, "{:?}", e);
}

#[test]
fn c11_client_key_runs_function_bindings_and_forwards_the_rest() {
    let (env, _dir, _) = server(CONFIG);
    let b = env.ok(&["run", "--", "cat"]);
    let s = env.json(&["ls", "--json"])[0]["session_id"].as_str().unwrap().to_string();
    let key = |k: &str| {
        let p = json!({ "session_id": s, "block_id": b, "key": k }).to_string();
        env.json(&["api", "call", "client.key", &p])
    };

    let r = key("ctrl+x");
    assert_eq!(r["handled"], true, "{r}");
    assert_eq!(env.json(&["ls", "--json"])[0]["label"], "bound");

    let r = key("ctrl+b");
    assert_eq!(r["pending"], true, "{r}");
    assert_eq!(r["prefix"], "ctrl+b", "{r}");

    let r = key("z");
    assert_eq!(r["handled"], false, "{r}");
    env.screen_until(&b, "z typed into cat", |s| s.contains('z'));
}

/// `leon do spin` in the background; killed on drop.
struct Bg(Child);

impl Drop for Bg {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
fn c13_a_looping_action_does_not_stall_the_server() {
    let (env, _dir, _) = server(CONFIG);
    let (s, _) = env.session();
    let _spin =
        Bg(env.cmd().args(["do", "-s", &s, "spin"]).stdout(Stdio::null()).stderr(Stdio::null()).spawn().unwrap());
    let end = Instant::now() + Duration::from_secs(10);
    while env.json(&["ls", "--json"])[0]["label"] != "spinning" {
        assert!(Instant::now() < end, "the spin action never started");
        std::thread::sleep(Duration::from_millis(50));
    }
    for args in [&["ls"][..], &["config", "reload"], &["keymap", "--json"]] {
        let t = Instant::now();
        env.ok(args);
        assert!(t.elapsed() < Duration::from_secs(2), "leon {args:?} took {:?} while an action spins", t.elapsed());
    }
}
