//! Impact proof for M5b: a failed reload keeps the old config's Lua thread serving its actions, and
//! a good reload swaps to the new one.

mod support;

use support::Env;

#[test]
fn a_failed_reload_keeps_running_the_old_actions() {
    let env = Env::new();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("init.lua");
    let action = |v: &str| format!("leon.action{{ name = 'which', title = 'W', run = function() return '{v}' end }}\n");
    std::fs::write(&path, action("v1")).unwrap();
    let out = env.cmd().env("LEON_CONFIG", &path).args(["server", "start"]).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert_eq!(env.ok(&["do", "which"]), "v1");

    std::fs::write(&path, "leon.action{ name = 'which', title = 'W', run = function() return 'v2' end\n").unwrap();
    assert_eq!(env.code(&["config", "reload"]), 1);
    assert_eq!(env.ok(&["do", "which"]), "v1", "the old thread still runs the old action");

    std::fs::write(&path, action("v2")).unwrap();
    env.ok(&["config", "reload"]);
    assert_eq!(env.ok(&["do", "which"]), "v2");
}
