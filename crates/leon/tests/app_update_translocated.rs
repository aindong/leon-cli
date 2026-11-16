//! App update criterion 12 (.agent/plans/2026-11-16-app-update-notice.md): an app macOS runs from
//! a translocated copy (a quarantined download, moved to a random read-only path) says to move it
//! to Applications first, and installs nothing.

// macOS only: Leon.app and its updater exist only there
#![cfg(target_os = "macos")]

mod app_update_kit;
mod release_kit;
mod update_kit;

use app_update_kit::{after, app_at, until, Kit, T0};
use leon_app::updates::Act;
use serde_json::json;
use update_kit::Release;

#[test]
fn a_translocated_app_says_move_it_first() {
    let at = "private/var/folders/xy/T/AppTranslocation/0A1B2C3D-4E5F/d/Leon.app";
    let k = Kit::with(Release::new, || app_at(at), json!({}));
    let before = k.leon.state();
    let u = k.updater();
    u.daily(true, T0);
    until("the notice", 30, || u.notice().is_some());
    let n = u.notice().unwrap();
    assert!(n.text.contains("Move Leon.app to Applications first"), "{}", n.text);
    assert!(!n.actions.contains(&Act::Install), "{:?}", n.actions);
    u.install();
    after(2);
    assert!(k.srv.asked().iter().all(|a| a.starts_with("latest.json")), "no file fetched: {:?}", k.srv.asked());
    k.leon.assert_unchanged(before, "a translocated app");
}
