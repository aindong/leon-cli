//! App update criterion 1 (.agent/plans/2026-11-16-app-update-notice.md): Leon.app checks for a
//! release at most once a day. The time goes into its store before the check starts, so a restart
//! within the day, a failed check and a crash mid-check all wait for the next day; a failed check
//! draws nothing and asks again only then. Also criterion 15's second half: a debug build, which
//! every test is, checks on its own only when it is pointed at a release server.

// macOS only: Leon.app and its updater exist only there
#![cfg(target_os = "macos")]

mod app_update_kit;
mod release_kit;
mod update_kit;

use app_update_kit::{after, until, Kit, T0};
use leon_app::updates::DAY;
use update_kit::Mode;

#[test]
fn the_check_runs_at_most_once_a_day_across_restarts() {
    let k = Kit::new();
    let u = k.updater();
    u.daily(true, T0);
    until("the first check", 30, || k.manifests() == 1 && u.notice().is_some());
    u.daily(true, T0 + 60);
    drop(u);
    // a restart an hour later: a new updater on the same store
    let u = k.updater();
    u.daily(true, T0 + 3600);
    after(2);
    assert_eq!(k.manifests(), 1, "a restart within the day asks nothing: {:?}", k.srv.asked());
    drop(u);
    let u = k.updater();
    u.daily(true, T0 + DAY);
    until("the next day's check", 30, || k.manifests() == 2);
}

#[test]
fn a_failed_check_is_silent_and_waits_a_day() {
    let k = Kit::new();
    k.srv.set("latest.json", Mode::Status(500));
    let u = k.updater();
    u.daily(true, T0);
    until("the check", 30, || k.manifests() == 1 && u.last_error().is_some());
    after(3);
    assert_eq!(k.manifests(), 1, "no retry loop: {:?}", k.srv.asked());
    assert!(u.notice().is_none(), "a failed daily check draws nothing: {:?}", u.notice().map(|n| n.text));
    assert!(u.last_error().unwrap().contains("500"), "what the log line says: {:?}", u.last_error());
    drop(u);
    let u = k.updater();
    u.daily(true, T0 + 3600);
    after(2);
    assert_eq!(k.manifests(), 1, "a failure counts as the day's check: {:?}", k.srv.asked());
}

#[test]
fn a_check_killed_midway_still_counts_as_the_days() {
    let k = Kit::new();
    k.srv.set("latest.json", Mode::Stall);
    let u = k.updater();
    u.daily(true, T0);
    until("the request", 30, || k.manifests() == 1);
    // the app quits (or crashes) while the request hangs
    drop(u);
    let u = k.updater();
    u.daily(true, T0 + 60);
    after(2);
    assert_eq!(k.manifests(), 1, "the time was kept before the check: {:?}", k.srv.asked());
}

#[test]
fn a_debug_build_checks_on_its_own_only_when_pointed_at_a_release_server() {
    let k = Kit::new();
    // a curl that writes its command line down and fails: if this updater ever looked, the red run
    // shows it without reaching the network
    let bin = k.leon.home.join("quietbin");
    std::fs::create_dir(&bin).unwrap();
    let curl = bin.join("curl");
    std::fs::write(&curl, "#!/bin/sh\necho \"$*\" >> \"$(dirname \"$0\")/ran\"\nexit 7\n").unwrap();
    std::fs::set_permissions(&curl, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    let path = std::env::var("PATH").unwrap();
    std::env::set_var("PATH", format!("{}:{path}", bin.display()));
    std::env::remove_var("LEON_RELEASE_URL");
    std::env::remove_var("LEON_TEST_UPDATE");
    let u = k.updater();
    u.daily(true, T0);
    after(2);
    std::env::set_var("PATH", path);
    assert!(!bin.join("ran").exists(), "a test's app asked the real release server");
    assert!(u.notice().is_none());
}
