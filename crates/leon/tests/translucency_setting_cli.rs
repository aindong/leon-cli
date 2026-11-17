//! `leon config set translucency N` writes the value into init.lua, refuses one out of range with
//! the range in its message, and `server.settings` reports what is set.

mod support;

use std::path::Path;
use std::process::Output;

use support::Env;

fn config(env: &Env, path: &Path, args: &[&str]) -> Output {
    env.cmd().env("LEON_CONFIG", path).arg("config").args(args).output().unwrap()
}

fn text(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned() + &String::from_utf8_lossy(&o.stderr)
}

#[test]
fn a_bad_value_is_refused_and_the_file_left_alone() {
    let env = Env::new();
    let path = env.dir.path().join("init.lua");
    const INIT: &str = "-- mine\nleon.set(\"translucency\", 0.3)\n";
    std::fs::write(&path, INIT).unwrap();
    for bad in ["2", "0.81", "glass", "true"] {
        let o = config(&env, &path, &["set", "translucency", bad]);
        assert_eq!(o.status.code(), Some(1), "{bad}: {}", text(&o));
        for word in ["translucency", "0.8", "opaque"] {
            assert!(text(&o).contains(word), "{bad}: {} should say {word:?}", text(&o));
        }
        assert_eq!(std::fs::read_to_string(&path).unwrap(), INIT, "{bad} left the file alone");
    }
}

#[test]
fn a_value_is_written_listed_and_reported_by_the_server() {
    let env = Env::new();
    let path = env.dir.path().join("init.lua");
    std::fs::write(&path, "-- mine\n").unwrap();
    let get = |key: &str| {
        let o = config(&env, &path, &["get", key]);
        assert!(o.status.success(), "get {key}: {}", text(&o));
        String::from_utf8_lossy(&o.stdout).trim().to_string()
    };
    let default = get("translucency");
    assert!(default.parse::<f64>().is_ok_and(|d| d > 0. && d <= 0.8), "translucent by default: {default:?}");
    let all = String::from_utf8_lossy(&config(&env, &path, &["get"]).stdout).into_owned();
    assert!(all.contains(&format!("translucency = {default}")), "config get lists it: {all}");

    let out = env.cmd().env("LEON_CONFIG", &path).args(["server", "start"]).output().unwrap();
    assert!(out.status.success(), "server start: {}", String::from_utf8_lossy(&out.stderr));
    let o = config(&env, &path, &["set", "translucency", "0.5"]);
    assert!(o.status.success(), "{}", text(&o));
    let now = std::fs::read_to_string(&path).unwrap();
    assert!(now.starts_with("-- mine\n") && now.contains("leon.set(\"translucency\", 0.5)"), "{now}");
    assert_eq!(get("translucency"), "0.5");
    // the set reloaded the running server
    let r = env.json(&["api", "call", "server.settings"]);
    assert_eq!(r["settings"]["translucency"], 0.5, "{r}");
    assert!(r["sources"]["translucency"].as_str().is_some_and(|s| s.ends_with("init.lua:2")), "{r}");

    let o = config(&env, &path, &["set", "translucency", "0"]);
    assert!(o.status.success(), "{}", text(&o));
    assert_eq!(env.json(&["api", "call", "server.settings"])["settings"]["translucency"], 0.0, "0 is opaque");
}
