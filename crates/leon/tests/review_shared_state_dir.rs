//! Two servers (two runtime dirs) on one state dir: only the first saves and restores, so they
//! can't restore the same sessions twice or overwrite each other's save.

mod support;

use support::Env;

#[test]
fn a_second_server_on_the_same_state_dir_keeps_nothing() {
    let (first, second) = (Env::new(), Env::new());
    let state = first.state_dir();
    let on_shared = |env: &Env, args: &[&str]| {
        let out = env.cmd().env("LEON_STATE_DIR", &state).args(args).output().unwrap();
        assert!(out.status.success(), "leon {args:?}: {}", String::from_utf8_lossy(&out.stderr));
        String::from_utf8(out.stdout).unwrap()
    };
    on_shared(&first, &["server", "start"]);
    on_shared(&second, &["server", "start"]);
    let st: serde_json::Value = serde_json::from_str(&on_shared(&second, &["server", "status", "--json"])).unwrap();
    assert!(st["state_file"].is_null(), "the second server saves nothing: {st}");
    on_shared(&first, &["new", "kept"]);
    on_shared(&second, &["new", "other"]);
    on_shared(&first, &["server", "stop"]);
    on_shared(&second, &["server", "stop"]);
    on_shared(&first, &["server", "start"]);
    let ls: serde_json::Value = serde_json::from_str(&on_shared(&first, &["ls", "--json"])).unwrap();
    assert_eq!(ls[0]["label"], "kept", "the first server's save wasn't overwritten: {ls}");
    assert_eq!(ls.as_array().unwrap().len(), 1, "{ls}");
}
