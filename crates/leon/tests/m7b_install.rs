//! M7b criterion 2 (.agent/plans/2026-11-14-m7b-ship.md): scripts/install.sh installs the app and a
//! CLI symlink under a temp HOME (or --prefix), is idempotent, and --uninstall removes both.

// macOS only: Leon.app bundles (plutil, codesign, ~/Applications).
#![cfg(target_os = "macos")]

mod install_kit;

use install_kit::{bundle_id, version, Kit};

#[test]
fn install_is_idempotent_and_uninstalls() {
    let k = Kit::new();
    let want = version(&k.app.join("Contents/MacOS/leon")).unwrap();
    for round in 0..2 {
        let r = k.install(&[]);
        assert!(r.ok, "install round {round}: {}", r.out);
        assert!(r.out.contains(&*k.installed_app().display().to_string()), "output names the app: {}", r.out);
        assert!(r.out.contains(&*k.cli().display().to_string()), "output names the CLI: {}", r.out);
        assert_eq!(bundle_id(&k.installed_app()).as_deref(), Some("dev.leon.app"));
        let link = std::fs::read_link(k.cli()).expect("the CLI is a symlink");
        assert_eq!(link, k.installed_app().join("Contents/MacOS/leon"));
        assert_eq!(version(&k.cli()).as_deref(), Some(want.as_str()));
    }
    let r = k.install(&["--uninstall"]);
    assert!(r.ok, "uninstall: {}", r.out);
    assert!(!k.installed_app().exists(), "the app is gone");
    assert!(k.cli().symlink_metadata().is_err(), "the CLI link is gone");
    let again = k.install(&["--uninstall"]);
    assert!(again.ok, "uninstalling twice is fine: {}", again.out);
}

#[test]
fn prefix_puts_the_cli_under_it() {
    let k = Kit::new();
    let prefix = k.tmp.path().join("p");
    let p = prefix.to_str().unwrap();
    let r = k.install(&["--prefix", p]);
    assert!(r.ok, "{}", r.out);
    let cli = prefix.join("bin/leon");
    assert_eq!(std::fs::read_link(&cli).unwrap(), k.installed_app().join("Contents/MacOS/leon"));
    assert!(version(&cli).is_some());
    assert!(k.cli().symlink_metadata().is_err(), "nothing at ~/.local/bin with --prefix");
    let r = k.install(&["--uninstall", "--prefix", p]);
    assert!(r.ok, "{}", r.out);
    assert!(cli.symlink_metadata().is_err() && !k.installed_app().exists());
}

/// An app dragged to the Trash leaves a dangling CLI link; it is still ours.
#[test]
fn a_dangling_link_to_our_app_is_still_ours() {
    let k = Kit::new();
    assert!(k.install(&[]).ok);
    std::fs::remove_dir_all(k.installed_app()).unwrap();
    let r = k.install(&[]);
    assert!(r.ok, "reinstall over a dangling link: {}", r.out);
    assert!(version(&k.cli()).is_some());
    std::fs::remove_dir_all(k.installed_app()).unwrap();
    let r = k.install(&["--uninstall"]);
    assert!(r.ok, "uninstall with the app gone: {}", r.out);
    assert!(k.cli().symlink_metadata().is_err(), "the dangling link is removed");
}

/// Found in review: HOME with and without a trailing slash is the same home, so the link stays ours.
#[test]
fn a_trailing_slash_in_home_is_the_same_home() {
    let k = Kit::new();
    let slashed = format!("{}/", k.home.display());
    let r = k.install_cmd(&[]).env("HOME", &slashed).output().unwrap();
    assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
    let r = k.install(&[]);
    assert!(r.ok, "reinstall without the slash: {}", r.out);
    let r = k.install_cmd(&["--uninstall"]).env("HOME", &slashed).output().unwrap();
    assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
    assert!(k.cli().symlink_metadata().is_err() && !k.installed_app().exists());
}

#[test]
fn install_script_never_uses_sudo() {
    let s = std::fs::read_to_string(install_kit::repo().join("scripts/install.sh")).unwrap();
    assert!(!s.contains("sudo"), "install.sh must not use sudo");
}
