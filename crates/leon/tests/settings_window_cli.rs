//! Settings window, the parts that need the `leon` binary or the docs: the Agents section reads
//! what the real `leon --json skill status` prints, and the docs say what the window has.

/// The docs name every section and the search, and CHANGELOG.md has the entry.
#[test]
fn the_docs_describe_the_window() {
    let settings = include_str!("../../../docs/lua/settings.md");
    for part in ["General", "Appearance", "Terminal", "Notifications", "Shortcuts", "Agents"] {
        assert!(settings.contains(part), "docs/lua/settings.md names the {part} section");
    }
    assert!(settings.contains("Its search\nfield finds a setting by its label or by its name here"), "the search");
    assert!(settings.contains("a control for every setting in the table below"), "every setting has a control");
    let shortcuts = include_str!("../../../docs/shortcuts.md");
    let row = shortcuts.lines().find(|l| l.starts_with("| ⌘, |")).expect("the ⌘, row");
    assert!(row.contains("every setting in sections, with search"), "{row}");
    let changelog = include_str!("../../../CHANGELOG.md");
    let unreleased = &changelog[changelog.find("## [Unreleased]").unwrap()..];
    let next = unreleased[3..].find("\n## ").map_or(unreleased.len(), |i| i + 3);
    assert!(unreleased[..next].contains("- **A Settings window with sections**"), "CHANGELOG.md [Unreleased]");
    let detail = include_str!("../../../docs/changelog-detail.md");
    assert!(detail.contains("### A Settings window with sections"), "its detail");
}

/// The window's parser reads the real `leon --json skill status`, and says to set up each agent
/// that isn't: here none is, in an empty home.
// macOS only: the Settings window (and so its parser) is in the app, which builds only there
#[cfg(target_os = "macos")]
#[test]
fn agents_parse_the_real_skill_status() {
    let home = tempfile::tempdir().unwrap();
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_leon"))
        .args(["--json", "skill", "status"])
        .env("HOME", home.path())
        .env_remove("CLAUDE_CONFIG_DIR")
        .env_remove("CODEX_HOME")
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let rows = leon_app::settings_window::parse_agents(&String::from_utf8_lossy(&out.stdout)).expect("it parses");
    let names: Vec<&str> = rows.iter().map(|r| r.agent.as_str()).collect();
    assert_eq!(names, ["claude", "codex"]);
    for r in &rows {
        assert_eq!(r.status, "missing", "{r:?}");
        assert_eq!(r.setup, Some(format!("leon skill setup --agent {}", r.agent)));
        assert!(r.skill.starts_with(&home.path().display().to_string()), "{r:?}");
    }
}
