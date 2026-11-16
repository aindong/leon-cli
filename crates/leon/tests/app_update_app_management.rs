//! App update criterion 13 (.agent/plans/2026-11-16-app-update-notice.md): when macOS's App
//! Management setting refuses the rename that replaces Leon.app ("Operation not permitted"), the
//! notice says what to do, with System Settings' pane and the release page, not the raw error.

// macOS only: Leon.app and its updater exist only there
#![cfg(target_os = "macos")]

use leon_app::updates::{Act, Phase};

#[test]
fn operation_not_permitted_on_the_replace_names_app_management() {
    let raw = "can't replace /Applications/Leon.app: Operation not permitted (os error 1); nothing was changed";
    let n = Phase::Failed(raw.into()).notice().expect("a failed install is shown");
    assert!(n.text.contains("App Management"), "{}", n.text);
    assert!(!n.text.contains("os error"), "not the raw error: {}", n.text);
    assert!(n.actions.contains(&Act::AppManagement) && n.actions.contains(&Act::ReleasePage), "{:?}", n.actions);
}

#[test]
fn another_failure_says_what_failed_and_offers_the_release_page() {
    let raw = "could not connect to github.com";
    let n = Phase::Failed(raw.into()).notice().unwrap();
    assert!(n.text.contains(raw), "{}", n.text);
    assert!(!n.actions.contains(&Act::AppManagement), "{:?}", n.actions);
    assert!(n.actions.contains(&Act::ReleasePage), "{:?}", n.actions);
}
