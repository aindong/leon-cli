//! Release kit criteria 1–2 (.agent/plans/2026-11-19-release-kit.md): on the first launch after
//! the version changes, a one-time notice "Updated to leon X.Y.Z." opens the website's What's new
//! page for that version; a first install, or a second launch, shows nothing.

mod m10b_support;

use std::sync::Arc;

use gpui::TestAppContext;
use leon_app::updates::{Act, Phase, Updater};
use m10b_support::*;

const VERSION: &str = env!("CARGO_PKG_VERSION");

/// A private state dir, as leon keeps its own (0700), and the updater's file in it.
fn state() -> (tempfile::TempDir, std::path::PathBuf) {
    use std::os::unix::fs::PermissionsExt;
    let d = tempfile::tempdir().unwrap();
    std::fs::set_permissions(d.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    let store = d.path().join("updates.json");
    (d, store)
}

fn updater(store: &std::path::Path) -> Updater {
    let find = Arc::new(|| Err("no install in this test".to_string()));
    Updater::new(Some(store.to_path_buf()), find, Arc::new(|| {}))
}

#[test]
fn the_first_launch_of_a_new_version_says_so_once() {
    let (_d, store) = state();
    // an earlier version's file: it looked for updates, and knows nothing of `seen`
    std::fs::write(&store, r#"{"checked_at":1790000000,"dismissed":null}"#).unwrap();
    let notice = updater(&store).notice().expect("a notice on the first launch after an update");
    assert_eq!(notice.text, format!("Updated to leon {VERSION}."));
    let page = format!("https://aindong.github.io/leon-cli/whats-new/{VERSION}/");
    assert_eq!(notice.actions, [Act::WhatsNew(page), Act::Dismiss]);
    assert!(updater(&store).notice().is_none(), "once: the next launch says nothing");

    std::fs::write(&store, r#"{"checked_at":1790000000,"seen":"0.0.1"}"#).unwrap();
    assert!(updater(&store).notice().is_some(), "a file from another version");
}

#[test]
fn a_first_install_says_nothing() {
    let (_d, store) = state();
    assert!(updater(&store).notice().is_none(), "no file: a first install, not an update");
    assert!(std::fs::read_to_string(&store).unwrap().contains(VERSION), "the version is kept");
    assert!(updater(&store).notice().is_none());
}

#[test]
fn an_unreadable_file_is_not_an_update() {
    let (_d, store) = state();
    std::fs::write(&store, b"").unwrap();
    assert!(updater(&store).notice().is_none(), "an empty or broken file says nothing");
}

#[gpui::test]
fn whats_new_is_a_button_and_a_palette_row(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    ws.read_with(cx, |ws, _| ws.updater().set_phase(Phase::Updated(VERSION.into())));
    cx.run_until_parked();
    assert!(drawn(cx, "update-notice") && drawn(cx, "update-whats-new") && drawn(cx, "update-dismiss"));
    let rows = ws.read_with(cx, |ws, _| ws.palette_rows_for("what"));
    assert!(rows.iter().any(|(t, _)| t == "Open What's New"), "{rows:?}");
    act(&ws, cx, "client.update.dismiss");
    assert!(!drawn(cx, "update-notice"), "dismissed");
    s.app.stop();
}
