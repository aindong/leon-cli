//! App update criterion 7 (.agent/plans/2026-11-16-app-update-notice.md): a Leon.app that Homebrew
//! installed shows the brew command, copyable, instead of Install, and is never replaced.

// macOS only: Leon.app and its updater exist only there
#![cfg(target_os = "macos")]

mod app_update_kit;
mod release_kit;
mod update_kit;

use app_update_kit::{after, app_at, until, Kit, T0};
use leon_app::updates::Act;
use serde_json::json;
use update_kit::Release;

const CASK: &str = "brew upgrade --cask aindong/leon/leon";

#[test]
fn a_cask_app_shows_the_brew_command_and_installs_nothing() {
    let k = Kit::with(Release::new, || app_at("brew/Caskroom/leon/0.0.1/Leon.app"), json!({}));
    let before = k.leon.state();
    let u = k.updater();
    u.daily(true, T0);
    until("the notice", 30, || u.notice().is_some());
    let n = u.notice().unwrap();
    assert!(n.text.contains(CASK), "the command: {}", n.text);
    assert!(n.actions.contains(&Act::Copy(CASK.into())), "copyable: {:?}", n.actions);
    assert!(!n.actions.contains(&Act::Install), "no Install: {:?}", n.actions);
    u.install();
    after(2);
    assert!(k.srv.asked().iter().all(|a| a.starts_with("latest.json")), "no file fetched: {:?}", k.srv.asked());
    k.leon.assert_unchanged(before, "a Homebrew app");
}
