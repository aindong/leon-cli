//! A relative LEON_STATE_DIR means one under the directory `server start` ran in, though the
//! spawned server runs in `/`.

mod support;

use support::Env;

#[test]
fn a_relative_state_dir_is_taken_from_where_start_ran() {
    let env = Env::new();
    let out = env
        .cmd()
        .current_dir(env.dir.path())
        .env("LEON_STATE_DIR", "rel-state")
        .args(["server", "start"])
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let st = env.json(&["server", "status", "--json"]);
    let want = std::fs::canonicalize(env.dir.path()).unwrap().join("rel-state/sessions.json");
    let got =
        std::fs::canonicalize(std::path::Path::new(st["state_file"].as_str().unwrap_or("/nope")).parent().unwrap());
    assert_eq!(got.ok().map(|d| d.join("sessions.json")), Some(want), "{st}");
}
