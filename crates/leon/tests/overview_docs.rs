//! Overview criterion 16 (.agent/plans/2026-11-17-overview-grid.md): the overview's keys and the
//! hover previews are documented where a user would look.

#[test]
fn the_overview_is_documented() {
    let readme = include_str!("../../../README.md");
    let row = readme.lines().find(|l| l.starts_with("| ⌘⇧\\, ⌘\\ |")).expect("the README's table lists ⌘⇧\\ and ⌘\\");
    assert!(row.contains("Overview") && row.contains("this session"), "{row}");
    let arch = include_str!("../../../docs/architecture/attention.md");
    for word in ["client.overview.toggle", "client.overview.session.toggle", "Reduce Motion", "hover"] {
        assert!(arch.contains(word), "docs/architecture/attention.md covers {word:?}");
    }
    let changelog = include_str!("../../../CHANGELOG.md");
    let unreleased = changelog.split("## [Unreleased]").nth(1).unwrap().split("\n## [").next().unwrap();
    assert!(unreleased.contains("⌘⇧\\"), "CHANGELOG.md's Unreleased section has the overview");
}
