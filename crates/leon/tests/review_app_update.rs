//! Review finding (.agent/plans/2026-11-16-app-update-notice.md, criterion 8): Check for Updates
//! after an install keeps the done notice and its Restart. The release a look finds then is the one
//! already installed and waiting for the restart; offering Install again would replace the app twice.

// macOS only: Leon.app and its updater exist only there
#![cfg(target_os = "macos")]

mod app_update_kit;
mod release_kit;
mod update_kit;

use app_update_kit::{after, until, Kit, T0};
use leon_app::updates::{Act, Phase};
use serde_json::json;
use update_kit::{Install, Mode, Release};

#[test]
fn check_for_updates_after_an_install_keeps_restart() {
    let k = Kit::new();
    let u = k.updater();
    u.daily(true, T0);
    until("the notice", 30, || u.notice().is_some_and(|n| n.actions.contains(&Act::Install)));
    u.install();
    until("installed", 60, || u.notice().is_some_and(|n| n.text.contains("installed")));
    u.check_now();
    after(2);
    let n = u.notice().expect("a notice");
    assert!(
        n.actions.contains(&Act::Restart) && !n.actions.contains(&Act::Install),
        "Restart lost after Check for Updates: {} {:?}",
        n.text,
        n.actions
    );
}

/// Review finding: a dismissed version hid the answer to a Check for Updates the user asked for
/// ("Looking…", then nothing). Asked for, it is shown again.
#[test]
fn check_for_updates_shows_a_dismissed_release() {
    let k = Kit::new();
    let u = k.updater();
    u.daily(true, T0);
    until("the notice", 30, || u.notice().is_some_and(|n| n.actions.contains(&Act::Install)));
    u.dismiss();
    assert!(u.notice().is_none());
    u.check_now();
    until("the answer", 30, || u.notice().is_some_and(|n| !n.text.contains("Looking")));
    assert!(u.notice().unwrap().actions.contains(&Act::Install), "{:?}", u.notice());
}

/// Review finding: Check for Updates while the daily look was still running was dropped without a
/// word. It shows "Looking…" now, and the running look's answer is shown as an asked one.
#[test]
fn check_for_updates_during_the_daily_look_is_answered() {
    let k = Kit::with(Release::new, Install::app, json!({ "timeout_secs": 3 }));
    k.srv.set("latest.json", Mode::Stall);
    let u = k.updater();
    u.daily(true, T0);
    until("the daily request", 30, || k.manifests() == 1);
    u.check_now();
    assert!(u.notice().is_some(), "Check for Updates while the daily look runs shows nothing");
    // the stalled look fails after its timeout: asked for, the failure is shown
    until("the answer", 30, || u.notice().is_some_and(|n| n.text.contains("failed")));
}

/// Review finding: a daily look that failed (or found nothing new) replaced the notice on screen,
/// so an app left open after an install lost its Restart.
#[test]
fn a_failed_daily_look_keeps_the_notice_on_screen() {
    let k = Kit::new();
    k.srv.set("latest.json", Mode::Status(500));
    let u = k.updater();
    u.set_phase(Phase::Failed("can't replace /x/Leon.app: Operation not permitted (os error 1)".into()));
    u.daily(true, T0);
    until("the look to fail", 30, || u.last_error().is_some());
    after(1);
    assert!(u.notice().is_some_and(|n| n.actions.contains(&Act::AppManagement)), "a failed daily look hid the notice");
}
