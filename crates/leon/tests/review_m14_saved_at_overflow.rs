//! Review fix (M14): `saved_at` comes from the state file; a far-future value must not crash the
//! restore while formatting the "restored from" line.

mod support;

use serde_json::{json, Value};
use support::Env;

#[test]
fn a_far_future_saved_at_doesnt_crash_the_restore() {
    let env = Env::new();
    env.ok(&["server", "start"]);
    let b = env.ok(&["run", "--", "/bin/sh"]);
    env.ok(&["server", "stop"]);
    let file = env.state_dir().join("sessions.json");
    let mut saved: Value = serde_json::from_slice(&std::fs::read(&file).unwrap()).unwrap();
    saved["saved_at"] = json!(67_768_000_000_000_000u64); // a year past i32::MAX - 1900 + 1900
    saved["sessions"][0]["blocks"][0]["scrollback"] = json!("some text");
    std::fs::write(&file, serde_json::to_vec(&saved).unwrap()).unwrap();
    let _ = env.run(&["server", "start"]);
    let out = env.run(&["--autostart=false", "block", "inspect", &b]);
    assert!(out.status.success(), "SERVER_DIED_RESTORING: {}", String::from_utf8_lossy(&out.stderr));
}
