//! M15 criterion 1 (.agent/plans/2026-11-15-m15-linux-server-cli.md): a `leon` built without the
//! app (Linux, or `--no-default-features` on macOS) prints its help when run alone, and `leon app`
//! fails saying so, starting no server.

#![cfg(not(all(feature = "app", target_os = "macos")))]

mod support;

use support::Env;

#[test]
fn leon_alone_prints_help() {
    let env = Env::new();
    let out = env.run(&[]);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert_eq!(out.status.code(), Some(0), "leon: {}", String::from_utf8_lossy(&out.stderr));
    assert!(stdout.contains("Usage: leon"), "prints the help: {stdout}");
    assert!(std::fs::symlink_metadata(env.socket()).is_err(), "no server was started");
}

#[test]
fn leon_app_says_the_app_is_not_built_in() {
    let env = Env::new();
    let out = env.run(&["app"]);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(1), "leon app: {stderr}");
    assert!(stderr.contains("without the app"), "says why: {stderr}");
    assert!(std::fs::symlink_metadata(env.socket()).is_err(), "no server was started");
}
