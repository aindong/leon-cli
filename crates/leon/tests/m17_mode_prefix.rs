//! M17 criterion 13: `leon.mode` takes a `prefix` (one chord), and the keymap reports it.

mod support;

use support::Env;

fn check(env: &Env, lua: &str) -> std::process::Output {
    let path = env.dir.path().join("check.lua");
    std::fs::write(&path, lua).unwrap();
    env.cmd().args(["config", "check"]).arg(&path).output().unwrap()
}

#[test]
fn mode_prefix_is_checked_and_reported() {
    let env = Env::new();
    let good = check(&env, "leon.mode(\"attach\", { prefix = \"ctrl+a\" })\n");
    assert!(good.status.success(), "{}", String::from_utf8_lossy(&good.stdout));
    for bad in ["ctrl+a>b", "nope+x"] {
        let out = check(&env, &format!("leon.mode(\"attach\", {{ prefix = \"{bad}\" }})\n"));
        let text = String::from_utf8_lossy(&out.stdout).to_string() + &String::from_utf8_lossy(&out.stderr);
        assert!(!out.status.success(), "prefix {bad:?} must be refused:\n{text}");
        assert!(text.contains("prefix"), "the diagnostic names the option:\n{text}");
    }

    let path = env.dir.path().join("init.lua");
    std::fs::write(&path, "leon.mode(\"attach\", { prefix = \"ctrl+a\" })\n").unwrap();
    let mut start = env.cmd();
    start.env("LEON_CONFIG", &path).args(["server", "start"]);
    assert!(start.status().unwrap().success());
    let km = env.json(&["keymap", "--json"]);
    let modes = km["modes"].as_array().unwrap();
    let attach = modes.iter().find(|m| m["name"] == "attach").unwrap_or_else(|| panic!("no attach mode: {km}"));
    assert_eq!(attach["prefix"], "ctrl+a", "{km}");
}
