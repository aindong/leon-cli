//! App update criterion 5 (.agent/plans/2026-11-16-app-update-notice.md): the notice shows only
//! for a newer release whose manifest verifies, with its version and release notes, and the app
//! downloads and replaces nothing on its own.

// macOS only: Leon.app and its updater exist only there
#![cfg(target_os = "macos")]

mod app_update_kit;
mod release_kit;
mod update_kit;

use app_update_kit::{after, until, Kit, T0};
use leon_app::updates::Act;
use serde_json::json;
use update_kit::{Install, Release, VERSION};

#[test]
fn a_newer_verified_release_shows_its_version_and_notes_and_nothing_is_fetched() {
    let k = Kit::new();
    let before = k.leon.state();
    let u = k.updater();
    u.daily(true, T0);
    until("the notice", 30, || u.notice().is_some());
    let n = u.notice().unwrap();
    assert!(n.text.contains(&format!("leon {VERSION} is available")), "{}", n.text);
    let notes = k.rel.manifest()["notes_url"].as_str().unwrap().to_string();
    assert!(n.actions.contains(&Act::Notes(notes.clone())), "the release notes, {notes}: {:?}", n.actions);
    assert!(n.actions.contains(&Act::Install) && n.actions.contains(&Act::Dismiss), "{:?}", n.actions);
    after(2);
    assert_eq!(k.srv.asked(), ["latest.json", "latest.json.minisig"], "only the manifest was fetched");
    k.leon.assert_unchanged(before, "a check alone");
}

#[test]
fn the_same_version_shows_nothing() {
    let k = Kit::with(Release::new, Install::app, json!({ "version": VERSION }));
    let u = k.updater();
    u.daily(true, T0);
    until("the check", 30, || k.srv.asked().len() == 2);
    after(1);
    assert!(u.notice().is_none(), "up to date draws nothing: {:?}", u.notice().map(|n| n.text));
}

#[test]
fn a_manifest_that_does_not_verify_shows_nothing() {
    let rel = Release::new();
    let mut m = rel.manifest();
    m["notes_url"] = json!("https://example.com/changed");
    rel.put("latest.json", m.to_string().as_bytes());
    let k = Kit::with(|| rel, Install::app, json!({}));
    let u = k.updater();
    u.daily(true, T0);
    until("the check to fail", 30, || u.last_error().is_some());
    assert!(u.notice().is_none(), "a bad signature draws nothing: {:?}", u.notice().map(|n| n.text));
    assert!(u.last_error().unwrap().contains("signature"), "{:?}", u.last_error());
}
