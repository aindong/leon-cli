//! `leon config set fade_inactive_panes N` writes the strength into init.lua, refuses a value out
//! of range with the range in its message, and `server.settings` reports what is set.

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
fn a_bad_strength_is_refused_and_the_file_left_alone() {
    let env = Env::new();
    let path = env.dir.path().join("init.lua");
    const INIT: &str = "-- mine\nleon.set(\"fade_inactive_panes\", 0.3)\n";
    std::fs::write(&path, INIT).unwrap();
    for bad in ["2", "0.81", "0.9", "strong", "true"] {
        let o = config(&env, &path, &["set", "fade_inactive_panes", bad]);
        assert_eq!(o.status.code(), Some(1), "{bad}: {}", text(&o));
        for word in ["fade_inactive_panes", "0.8", "off"] {
            assert!(text(&o).contains(word), "{bad}: {} should say {word:?}", text(&o));
        }
        assert_eq!(std::fs::read_to_string(&path).unwrap(), INIT, "{bad} left the file alone");
    }
}

#[test]
fn a_strength_is_written_listed_and_reported_by_the_server() {
    let env = Env::new();
    let path = env.dir.path().join("init.lua");
    std::fs::write(&path, "-- mine\n").unwrap();
    let get = |key: &str| {
        let o = config(&env, &path, &["get", key]);
        assert!(o.status.success(), "get {key}: {}", text(&o));
        String::from_utf8_lossy(&o.stdout).trim().to_string()
    };
    let default = get("fade_inactive_panes");
    assert!(default.parse::<f64>().is_ok_and(|d| d > 0. && d <= 0.8), "the default is on: {default:?}");
    let all = String::from_utf8_lossy(&config(&env, &path, &["get"]).stdout).into_owned();
    assert!(all.contains(&format!("fade_inactive_panes = {default}")), "config get lists it: {all}");

    let out = env.cmd().env("LEON_CONFIG", &path).args(["server", "start"]).output().unwrap();
    assert!(out.status.success(), "server start: {}", String::from_utf8_lossy(&out.stderr));
    let o = config(&env, &path, &["set", "fade_inactive_panes", "0.5"]);
    assert!(o.status.success(), "{}", text(&o));
    let now = std::fs::read_to_string(&path).unwrap();
    assert!(now.starts_with("-- mine\n") && now.contains("leon.set(\"fade_inactive_panes\", 0.5)"), "{now}");
    assert_eq!(get("fade_inactive_panes"), "0.5");
    // the set reloaded the running server
    let r = env.json(&["api", "call", "server.settings"]);
    assert_eq!(r["settings"]["fade_inactive_panes"], 0.5, "{r}");
    assert!(r["sources"]["fade_inactive_panes"].as_str().is_some_and(|s| s.ends_with("init.lua:2")), "{r}");

    let o = config(&env, &path, &["set", "fade_inactive_panes", "0"]);
    assert!(o.status.success(), "{}", text(&o));
    assert_eq!(env.json(&["api", "call", "server.settings"])["settings"]["fade_inactive_panes"], 0.0, "0 is off");
}
