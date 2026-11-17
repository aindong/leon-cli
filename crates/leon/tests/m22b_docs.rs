//! M22b criterion 7: the docs say the app uses themes and settings now, and how to open Settings.

#[test]
fn m22b_documented() {
    let readme = include_str!("../../../docs/shortcuts.md");
    assert!(readme.lines().any(|l| l.starts_with("| ⌘, |") && l.contains("Settings")), "docs/shortcuts.md's table: ⌘,");
    let cli = include_str!("../../../docs/cli/settings.md");
    assert!(!cli.contains("next release"), "docs/cli/settings.md still says themes come later");
    assert!(cli.contains("⌘,"), "docs/cli/settings.md points at the Settings window");
    let arch = include_str!("../../../docs/architecture/state-and-hosts.md");
    assert!(arch.contains("(M22b)"), "docs/architecture has the app half of the settings decision");
    let changelog = include_str!("../../../docs/changelog-detail.md");
    assert!(changelog.contains("### M22b"), "the changelog's detail has an M22b entry");
}
