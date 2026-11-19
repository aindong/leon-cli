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
    // its links point at the tag (release_notes_links.rs)
    let blob = format!("](https://github.com/aindong/leon-cli/blob/v{VERSION}/");
    let want = section(&changelog, VERSION)
        .replace("](docs/", &format!("{blob}docs/"))
        .replace("](README.md", &format!("{blob}README.md"));
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

#[test]
fn an_earlier_release_comes_from_the_archive_with_its_links_from_the_root() {
    let archive = std::fs::read_to_string(repo().join("docs/changelog-archive.md")).unwrap();
    assert!(section(&archive, "0.1.0").len() > 200, "the archive has 0.1.0's section");
    let o = script("release-notes.sh", &["0.1.0"], &[]);
    assert!(o.status.success(), "{}", text(&o));
    let notes = String::from_utf8_lossy(&o.stdout);
    let blob = "](https://github.com/aindong/leon-cli/blob/v0.1.0/";
    assert!(notes.starts_with("The first release of leon"), "{notes}");
    assert!(notes.contains(&format!("{blob}docs/changelog-milestones.md)")), "docs/ links: {notes}");
    assert!(notes.contains(&format!("{blob}README.md#install)")), "../ links: {notes}");
    assert!(!notes.contains("](changelog-") && !notes.contains("../"), "no link left relative: {notes}");
}

#[test]
fn an_archived_sections_links_of_every_kind_come_back_from_the_root() {
    // the script reads the archive beside it: a copy in a repo of its own
    let tmp = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(tmp.path().join("scripts")).unwrap();
    std::fs::create_dir_all(tmp.path().join("docs")).unwrap();
    std::fs::copy(repo().join("scripts/release-notes.sh"), tmp.path().join("scripts/release-notes.sh")).unwrap();
    std::fs::write(tmp.path().join("CHANGELOG.md"), "# Changelog\n\n## [1.1.0]\n\n- new\n").unwrap();
    let archive = "## [1.0.0]\n\n[a](x.md) ![p](images/p.png) [r](../README.md#i) [u](https://e.x/y)\n\n\
                   [d]: x.md\n[w]: https://keepachangelog.com/\n";
    std::fs::write(tmp.path().join("docs/changelog-archive.md"), archive).unwrap();
    let o = std::process::Command::new("bash")
        .arg(tmp.path().join("scripts/release-notes.sh"))
        .arg("1.0.0")
        .output()
        .unwrap();
    assert!(o.status.success(), "{}", text(&o));
    let (blob, raw) =
        ("https://github.com/aindong/leon-cli/blob/v1.0.0", "https://github.com/aindong/leon-cli/raw/v1.0.0");
    let want = format!(
        "[a]({blob}/docs/x.md) ![p]({raw}/docs/images/p.png) [r]({blob}/README.md#i) [u](https://e.x/y)\n\n\
         [d]: {blob}/docs/x.md\n[w]: https://keepachangelog.com/"
    );
    assert_eq!(String::from_utf8_lossy(&o.stdout).trim(), want);
}
