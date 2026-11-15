//! Release pipeline criterion 2: a release's notes are its version's section of CHANGELOG.md
//! (`scripts/release-notes.sh`).

mod release_kit;

use release_kit::{repo, script, text, VERSION};

/// The lines under `## [version]`, up to the next `## [`: written apart from the script.
fn section(changelog: &str, version: &str) -> String {
    let head = format!("## [{version}]");
    let lines = changelog.lines().skip_while(|l| !l.starts_with(&head)).skip(1);
    lines.take_while(|l| !l.starts_with("## [")).collect::<Vec<_>>().join("\n").trim().to_string()
}

#[test]
fn notes_are_this_versions_changelog_section() {
    let changelog = std::fs::read_to_string(repo().join("CHANGELOG.md")).unwrap();
    let want = section(&changelog, VERSION);
    assert!(want.len() > 200, "CHANGELOG.md has a section for {VERSION}");
    let o = script("release-notes.sh", &[VERSION], &[]);
    assert!(o.status.success(), "{}", text(&o));
    assert_eq!(String::from_utf8_lossy(&o.stdout).trim(), want);
}

#[test]
fn notes_stop_at_the_next_section_and_a_missing_version_fails() {
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("CHANGELOG.md");
    let text_in = "# Changelog\n\n## [Unreleased]\n\n- soon\n\n## [1.2.0] - 2030-01-02\n\n### Added\n\n- two\n\n\
                   ## [1.1.0] - 2030-01-01\n\n- one\n\n## [1.10.0] - 2029-01-01\n\n- ten\n";
    std::fs::write(&file, text_in).unwrap();
    let file = file.to_str().unwrap();
    let notes = |v: &str| script("release-notes.sh", &[v, file], &[]);

    let o = notes("1.2.0");
    assert!(o.status.success(), "{}", text(&o));
    assert_eq!(String::from_utf8_lossy(&o.stdout).trim(), "### Added\n\n- two");
    // 1.1.0 is not a prefix match for 1.10.0, and the dots are not wildcards
    assert_eq!(String::from_utf8_lossy(&notes("1.1.0").stdout).trim(), "- one");
    for missing in ["9.9.9", "1.1", "1x1.0", "Unreleased]"] {
        let o = notes(missing);
        assert!(!o.status.success(), "{missing} has no section, but release-notes.sh passed");
        assert!(o.stdout.is_empty(), "no notes for {missing}: {}", text(&o));
        assert!(String::from_utf8_lossy(&o.stderr).contains(missing), "the error names {missing}: {}", text(&o));
    }
}
