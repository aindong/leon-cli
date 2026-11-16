//! App update criteria 14 and 18 (.agent/plans/2026-11-16-app-update-notice.md): the docs say what
//! the app does about updates, the setting is listed, and the App Management finding is there.

fn read(path: &str) -> String {
    std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join(path)).unwrap()
}

#[test]
fn updates_md_describes_the_app() {
    let doc = read("docs/updates.md");
    for want in [
        "once a day",
        "check_for_updates",
        "Check for Updates",
        "Install Update",
        "Restart",
        "brew upgrade --cask aindong/leon/leon",
        "App Management",
        "Move Leon.app to Applications",
        "AppTranslocation",
    ] {
        assert!(doc.contains(want), "docs/updates.md says {want:?}");
    }
    assert!(!doc.contains("It never updates or checks on its own."), "the app does check on its own now");
    assert!(doc.contains("Team ID"), "Apple's rule for which apps may change a bundle");
    assert!(doc.contains("renamex_np"), "what the temp-dir run of part A's swap found");
    assert!(doc.len() < 20_000, "docs/updates.md stays under 20 KB: {}", doc.len());
}

#[test]
fn the_setting_is_listed() {
    let lua = read("docs/lua.md");
    assert!(lua.contains("| `check_for_updates` |"), "docs/lua.md's settings table");
    assert!(lua.len() < 20_000, "docs/lua.md stays under 20 KB: {}", lua.len());
    assert!(read("docs/cli/settings.md").contains("check_for_updates"));
}
