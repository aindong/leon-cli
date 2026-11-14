//! M8 acceptance (.agent/plans/2026-11-14-m8-persist-speed.md, criterion 12): `server status` says
//! where sessions are saved, or null when the state dir isn't private and nothing is saved.

mod support;

use std::os::unix::fs::{DirBuilderExt, PermissionsExt};

use support::Env;

#[test]
fn status_shows_the_state_file() {
    let env = Env::new();
    env.ok(&["server", "start"]);
    let st = env.json(&["server", "status", "--json"]);
    assert_eq!(st["state_file"], env.state_dir().join("sessions.json").to_str().unwrap());
}

#[test]
fn an_open_state_dir_saves_nothing_and_says_so() {
    let env = Env::new();
    std::fs::DirBuilder::new().mode(0o755).create(env.state_dir()).unwrap();
    std::fs::set_permissions(env.state_dir(), std::fs::Permissions::from_mode(0o755)).unwrap();
    env.ok(&["server", "start"]);
    let st = env.json(&["server", "status", "--json"]);
    assert!(st.get("state_file").is_some_and(|f| f.is_null()), "state_file is null: {st}");
    env.ok(&["new", "s"]);
    env.ok(&["server", "stop"]);
    assert!(!env.state_dir().join("sessions.json").exists(), "nothing saved into an open dir");
    let mode = std::fs::metadata(env.state_dir()).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o755, "never chmod a dir leon didn't create");
}
