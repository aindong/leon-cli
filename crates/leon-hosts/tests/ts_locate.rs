//! Tailscale criterion 10 (.agent/plans/2026-11-16-tailscale-hosts.md): which `tailscale` leon
//! runs. `LEON_TAILSCALE` wins, then `tailscale` on PATH, then the app bundle's binary (macOS).

use std::ffi::OsString;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use leon_hosts::tailscale::{locate, BUNDLES};

fn exe(p: &Path) {
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, "#!/bin/sh\n").unwrap();
    std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o755)).unwrap();
}

#[test]
fn env_then_path_then_the_app_bundle() {
    let d = tempfile::tempdir().unwrap();
    let (bin, other) = (d.path().join("bin"), d.path().join("other"));
    let bundle = d.path().join("Tailscale.app/Contents/MacOS/Tailscale");
    exe(&bundle);
    let path = |dirs: &[&Path]| Some(std::env::join_paths(dirs).unwrap());
    let none: Option<OsString> = None;

    // nothing on PATH: the bundle
    assert_eq!(locate(none.as_deref(), path(&[&other]).as_deref(), &[&bundle]), Some(bundle.clone()));
    // a tailscale on PATH comes first; a directory or a file that can't run doesn't count
    std::fs::create_dir_all(other.join("tailscale")).unwrap();
    std::fs::create_dir_all(&bin).unwrap();
    std::fs::write(bin.join("tailscale"), "").unwrap();
    assert_eq!(locate(none.as_deref(), path(&[&other, &bin]).as_deref(), &[&bundle]), Some(bundle.clone()));
    exe(&bin.join("tailscale"));
    assert_eq!(locate(none.as_deref(), path(&[&other, &bin]).as_deref(), &[&bundle]), Some(bin.join("tailscale")));
    // LEON_TAILSCALE wins, even when it names nothing (then Tailscale "isn't installed")
    let env = OsString::from("/nowhere/tailscale");
    assert_eq!(locate(Some(&env), path(&[&bin]).as_deref(), &[&bundle]), Some("/nowhere/tailscale".into()));
    // an empty one is unset
    assert_eq!(locate(Some(&OsString::new()), path(&[&bin]).as_deref(), &[]), Some(bin.join("tailscale")));
    // none at all
    assert_eq!(locate(none.as_deref(), path(&[&other]).as_deref(), &[&d.path().join("missing")]), None);
    // relative PATH entries are skipped (a cwd-planted tailscale)
    assert_eq!(locate(none.as_deref(), Some(&OsString::from("bin")), &[]), None);
}

#[test]
fn the_bundle_is_the_mac_app() {
    match cfg!(target_os = "macos") {
        true => assert_eq!(BUNDLES, ["/Applications/Tailscale.app/Contents/MacOS/Tailscale"]),
        false => assert!(BUNDLES.is_empty()),
    }
}
