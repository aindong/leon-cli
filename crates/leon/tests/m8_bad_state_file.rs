//! M8 acceptance (.agent/plans/2026-11-14-m8-persist-speed.md, criterion 4): a state file that isn't
//! JSON, is from a newer version, or doesn't add up starts the server empty, with a warning, and
//! the bad file is kept aside with its bytes.

mod support;

use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};

use serde_json::{json, Value};
use support::Env;

fn write_state(env: &Env, bytes: &[u8]) {
    let dir = env.state_dir();
    if !dir.exists() {
        std::fs::DirBuilder::new().mode(0o700).create(&dir).unwrap();
    }
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(dir.join("sessions.json"))
        .unwrap();
    std::io::Write::write_all(&mut f, bytes).unwrap();
}

/// Start on the bad file and check: running, empty, warned, file kept aside byte for byte.
fn starts_empty_and_keeps(env: &Env, bytes: &[u8], what: &str) {
    write_state(env, bytes);
    env.ok(&["server", "start"]);
    assert_eq!(env.code(&["--autostart=false", "server", "status"]), 0, "{what}: the server runs");
    assert_eq!(env.json(&["ls", "--json"]), json!([]), "{what}: no sessions");
    let log = std::fs::read_to_string(env.dir.path().join("server.log")).unwrap_or_default();
    let file = env.state_dir().join("sessions.json");
    assert!(
        log.lines().any(|l| l.contains("warning") && l.contains(file.to_str().unwrap())),
        "{what}: the log warns about {}:\n{log}",
        file.display()
    );
    let aside: Vec<_> = std::fs::read_dir(env.state_dir())
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.file_name().unwrap().to_string_lossy().starts_with("sessions.json.bad-"))
        .collect();
    assert_eq!(aside.len(), 1, "{what}: one bad file kept aside, got {aside:?}");
    assert_eq!(std::fs::read(&aside[0]).unwrap(), bytes, "{what}: kept with its original bytes");
}

#[test]
fn not_json_starts_empty() {
    starts_empty_and_keeps(&Env::new(), b"{not json", "not JSON");
}

#[test]
fn newer_version_starts_empty() {
    starts_empty_and_keeps(&Env::new(), br#"{"version":999,"sessions":[]}"#, "version 999");
}

#[test]
fn a_layout_naming_a_missing_block_starts_empty() {
    let env = Env::new();
    env.ok(&["server", "start"]);
    env.ok(&["new", "s"]);
    env.ok(&["server", "stop"]);
    let file = env.state_dir().join("sessions.json");
    let mut v: Value = serde_json::from_slice(&std::fs::read(&file).unwrap()).unwrap();
    // The layout still names the block, but the saved block now has another ID.
    let id = &mut v["sessions"][0]["blocks"][0]["block_id"];
    assert!(id.is_string(), "the saved file has sessions[0].blocks[0].block_id: {v}");
    *id = json!("block:00000000-0000-0000-0000-000000000000");
    starts_empty_and_keeps(&env, &serde_json::to_vec(&v).unwrap(), "inconsistent layout");
}
