//! Overview criterion 16 (.agent/plans/2026-11-17-overview-grid.md): the overview's keys and the
//! hover previews are documented where a user would look.

#[test]
fn the_overview_is_documented() {
    let shortcuts = include_str!("../../../docs/shortcuts.md");
    let row = shortcuts
        .lines()
        // Markdown would eat a lone `\` before `,` or `:`, so the page writes `\\`
        .find(|l| l.starts_with("| ⌘⇧\\\\, ⌘\\\\ |"))
        .expect("docs/shortcuts.md's table lists ⌘⇧\\ and ⌘\\");
    assert!(row.contains("Overview") && row.contains("this session"), "{row}");
    let arch = include_str!("../../../docs/architecture/attention.md");
    for word in ["client.overview.toggle", "client.overview.session.toggle", "Reduce Motion", "hover"] {
        assert!(arch.contains(word), "docs/architecture/attention.md covers {word:?}");
    }
    // 0.1.0's section moved to the archive when 0.1.1 came out
    let changelog = include_str!("../../../docs/changelog-archive.md");
    let release = changelog.split("## [0.1.0]").nth(1).unwrap().split("\n## [").next().unwrap();
    assert!(release.contains("⌘⇧\\"), "the changelog's 0.1.0 section has the overview");
}
