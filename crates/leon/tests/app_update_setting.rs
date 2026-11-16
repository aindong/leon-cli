//! App update criterion 3 (.agent/plans/2026-11-16-app-update-notice.md):
//! `leon.set("check_for_updates", false)` turns Leon.app's daily check off. It defaults to true,
//! takes only a boolean, `leon config set` writes it, and `server.settings` reports it.

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
fn it_defaults_on_takes_a_boolean_and_the_server_reports_it() {
    let env = Env::new();
    let path = env.dir.path().join("init.lua");
    std::fs::write(&path, "-- mine\n").unwrap();
    let get = config(&env, &path, &["get", "check_for_updates"]);
    assert!(get.status.success(), "{}", text(&get));
    assert_eq!(String::from_utf8_lossy(&get.stdout).trim(), "true", "on by default");

    for bad in ["sometimes", "1", "0.5"] {
        let o = config(&env, &path, &["set", "check_for_updates", bad]);
        assert_eq!(o.status.code(), Some(1), "{bad}: {}", text(&o));
        assert!(text(&o).contains("check_for_updates"), "{bad}: {}", text(&o));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "-- mine\n", "{bad} left the file alone");
    }

    let out = env.cmd().env("LEON_CONFIG", &path).args(["server", "start"]).output().unwrap();
    assert!(out.status.success(), "server start: {}", String::from_utf8_lossy(&out.stderr));
    assert_eq!(env.json(&["api", "call", "server.settings"])["settings"]["check_for_updates"], true);
    let o = config(&env, &path, &["set", "check_for_updates", "false"]);
    assert!(o.status.success(), "{}", text(&o));
    assert!(std::fs::read_to_string(&path).unwrap().contains("leon.set(\"check_for_updates\", false)"));
    let r = env.json(&["api", "call", "server.settings"]);
    assert_eq!(r["settings"]["check_for_updates"], false, "{r}");
    assert!(r["sources"]["check_for_updates"].as_str().is_some_and(|s| s.ends_with("init.lua:2")), "{r}");
}
