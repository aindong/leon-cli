//! The Connect popover's docs (.agent/plans/2026-11-22-sidebar-footer-connect.md, criterion 8):
//! remote-hosts.md says how to connect from the sidebar's footer, shortcuts.md lists ⌘⇧K, and
//! CHANGELOG's Unreleased section names it.

fn read(path: &str) -> String {
    std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join(path)).unwrap()
}

#[test]
fn the_footer_and_connect_are_documented() {
    let hosts = read("docs/remote-hosts.md");
    assert!(hosts.contains("footer") && hosts.contains("Connect to a Machine…"), "remote-hosts.md");
    assert!(hosts.contains("tailnet://") && hosts.contains("On your tailnet"), "remote-hosts.md: the tailnet rows");
    let keys = read("docs/shortcuts.md");
    assert!(keys.lines().any(|l| l.starts_with("| ⌘⇧K |") && l.contains("Connect")), "shortcuts.md: ⌘⇧K");
    let log = read("CHANGELOG.md");
    let unreleased = log.split("## [Unreleased]").nth(1).and_then(|s| s.split("\n## [").next()).unwrap_or_default();
    assert!(unreleased.contains("Connect"), "CHANGELOG [Unreleased] names it");
}
