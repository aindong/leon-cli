//! Update criterion 18 (.agent/plans/2026-11-16-self-update-core.md): end to end for the app. A
//! Leon.app in a temp dir is replaced by the bundle in the served zip (made with scripts/bundle.sh
//! and zipped as scripts/release.sh does), its ad hoc signature still verifies, and its leon runs.

// macOS only: Leon.app, codesign and ditto exist only there
#![cfg(target_os = "macos")]

mod release_kit;
mod update_kit;

use std::process::Command;

use update_kit::{app_zip, name, run, Install, Release, VERSION};

fn ok(cmd: &mut Command) -> String {
    let o = cmd.output().unwrap();
    assert!(o.status.success(), "{cmd:?}: {}", release_kit::text(&o));
    release_kit::text(&o)
}

#[test]
fn an_app_install_updates_itself_to_the_served_release() {
    let rel = Release::unsigned();
    let built = rel.tmp.path().join("build/Leon.app");
    ok(Command::new("bash")
        .arg(release_kit::repo().join("scripts/bundle.sh"))
        .args(["--bin", env!("CARGO_BIN_EXE_leon"), "--out"])
        .arg(&built));
    ok(Command::new("codesign").args(["--verify", "--deep", "--strict", "--verbose=4"]).arg(&built));
    rel.put(&name("app"), &app_zip(&built));
    rel.sign();

    // the installed app: the same bundle, with a file only the old one has
    let leon = Install::app();
    std::fs::remove_dir_all(&leon.root).unwrap();
    ok(Command::new("ditto").arg(&built).arg(&leon.root));
    let marker = leon.root.join("Contents/Resources/only-in-the-old-app");
    std::fs::write(&marker, "old").unwrap();
    let before = leon.state();

    let srv = rel.serve();
    let o = run(leon.cmd(&rel, &srv).arg("self-update"), 600);
    assert_eq!(o.code, 0, "the update goes through: {}", o.text());
    assert!(o.out.contains(leon.root.to_str().unwrap()), "it names the app it replaced: {}", o.text());
    assert!(o.out.contains("leon server stop"), "it says what a running server needs: {}", o.text());

    assert_ne!(leon.state(), before, "the bundle was swapped, not written into");
    assert!(!marker.exists(), "the old bundle's files are gone");
    assert!(leon.leftovers().is_empty(), "no staging directory is left: {:?}", leon.leftovers());
    ok(Command::new("codesign").args(["--verify", "--deep", "--strict", "--verbose=4"]).arg(&leon.root));
    let version = leon.version();
    assert!(version.starts_with(&format!("leon {VERSION}")), "the new app's leon runs: {version}");
    assert!(leon.root.join("Contents/Resources/Leon.icns").is_file(), "with the rest of the bundle");
    let quarantine = Command::new("xattr").args(["-p", "com.apple.quarantine"]).arg(&leon.root).output().unwrap();
    assert!(!quarantine.status.success(), "the new app is not quarantined");
}
