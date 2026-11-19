//! Release kit criterion 2 (.agent/plans/2026-11-19-release-kit.md) and a review finding: the daily
//! look replaces the "Updated to" notice with a newer release's, but a release the user dismissed
//! leaves the "Updated to" notice where it is rather than putting nothing in its place.

// macOS only: Leon.app and its updater exist only there
#![cfg(target_os = "macos")]

mod app_update_kit;
mod release_kit;
mod update_kit;

use app_update_kit::{until, Kit, T0};
use update_kit::VERSION;

fn updated_then_look(saved: &str) -> String {
    let k = Kit::new();
    std::fs::write(&k.store, saved).unwrap();
    let u = k.updater();
    assert!(u.notice().unwrap().text.starts_with("Updated to leon"), "{:?}", u.notice());
    let woken = k.woken_on.lock().unwrap().len();
    u.daily(true, T0);
    until("the look to end", 30, || k.manifests() == 1 && k.woken_on.lock().unwrap().len() > woken);
    u.notice().map(|n| n.text).unwrap_or_default()
}

#[test]
fn a_newer_release_replaces_it() {
    let text = updated_then_look(r#"{"seen":"0.0.1"}"#);
    assert!(text.contains(&format!("leon {VERSION} is available")), "{text}");
}

#[test]
fn a_dismissed_release_leaves_it() {
    let text = updated_then_look(&format!(r#"{{"dismissed":"{VERSION}","seen":"0.0.1"}}"#));
    assert!(text.starts_with("Updated to leon"), "the notice went: {text:?}");
}
