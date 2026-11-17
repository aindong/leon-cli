//! App update criterion 8 (.agent/plans/2026-11-16-app-update-notice.md): Install runs part A's
//! install off the app's thread, the notice follows its steps, and it ends with the new version
//! installed and a Restart.

// macOS only: Leon.app and its updater exist only there
#![cfg(target_os = "macos")]

mod app_update_kit;
mod release_kit;
mod update_kit;

use std::time::{Duration, Instant};

use app_update_kit::{after, until, Kit, T0};
use leon_app::updates::Act;
use serde_json::json;
use update_kit::{Install, Release, VERSION};

#[test]
fn install_shows_each_step_then_restart() {
    let k = Kit::new();
    let before = k.leon.state();
    let u = k.updater();
    u.daily(true, T0);
    until("the notice", 30, || u.notice().is_some());
    let start = Instant::now();
    u.install();
    assert!(start.elapsed() < Duration::from_millis(500), "install() waited for the download");
    // the notice shows a moment before the wake that records it
    let recorded = || k.seen.lock().unwrap().last().is_some_and(|t| t.contains("installed"));
    until("installed", 60, recorded);
    let n = u.notice().unwrap();
    assert!(n.text.contains(&format!("leon {VERSION} is installed")) && n.text.contains("next launch"), "{}", n.text);
    assert!(n.actions.contains(&Act::Restart), "{:?}", n.actions);
    let seen = k.seen.lock().unwrap().clone();
    let at = |word: &str| seen.iter().position(|t| t.contains(word)).unwrap_or_else(|| panic!("{word}: {seen:?}"));
    let downloading = at("downloading");
    assert!(seen[downloading].contains("MB"), "{seen:?}");
    assert!(downloading < at("checking") && at("checking") < at("installing ") && at("installing ") < at("installed"));
    assert_ne!(k.leon.state(), before, "the bundle was replaced");
    assert!(k.leon.version().starts_with(&format!("leon {VERSION}")), "the new app's leon runs");
    assert!(k.woken_on.lock().unwrap().iter().all(|t| t == "leon-update"), "off the caller's thread");
}

#[test]
fn a_paused_install_says_installing() {
    let k = Kit::with(Release::new, Install::app, json!({ "pause": "installing" }));
    let u = k.updater();
    u.daily(true, T0);
    until("the notice", 30, || u.notice().is_some());
    u.install();
    until("installing", 60, || u.notice().is_some_and(|n| n.text.contains("installing")));
    after(1);
    let n = u.notice().unwrap();
    assert!(n.text.contains("installing") && !n.actions.contains(&Act::Install), "{} {:?}", n.text, n.actions);
}
