//! Update criterion 21 (.agent/plans/2026-11-16-self-update-core.md): the crates the updater
//! brings into the binary are in the third-party notices, each with its license.

mod release_kit;

#[test]
fn the_notices_list_the_updaters_crates() {
    let o = release_kit::script("notices.sh", &[], &[]);
    assert!(o.status.success(), "notices.sh: {}", release_kit::text(&o));
    let notices = String::from_utf8(o.stdout).unwrap();
    let mut crates = vec!["minisign-verify", "semver", "sha2", "tar", "flate2"];
    if cfg!(target_os = "macos") {
        crates.push("zip"); // the app's archive: only macOS unpacks one
    }
    for name in crates {
        let line = notices.lines().find(|l| l.starts_with(&format!("- {name} ")));
        let line = line.unwrap_or_else(|| panic!("{name} is not in the notices"));
        assert!(line.contains("MIT") || line.contains("Apache-2.0"), "{name} has its license: {line}");
    }
    assert!(notices.contains("Frank Denis"), "minisign-verify's license text is there, not only its name");
}
