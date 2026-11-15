//! M7b criterion 1 (.agent/plans/2026-11-14-m7b-ship.md): `scripts/bundle.sh --bin --out` makes a
//! valid Leon.app from the test binary, rebuilds over its own output, and refuses anything else.

// macOS only: Leon.app bundles (plutil, codesign, ~/Applications).
#![cfg(target_os = "macos")]

mod help_walk;

use std::path::Path;
use std::process::{Command, Output};

fn bundle(out: &Path) -> Output {
    Command::new("bash")
        .arg(help_walk::repo().join("scripts/bundle.sh"))
        .args(["--bin", env!("CARGO_BIN_EXE_leon"), "--out"])
        .arg(out)
        .output()
        .unwrap()
}

fn plist(app: &Path, key: &str) -> String {
    let o = Command::new("plutil")
        .args(["-extract", key, "raw", "-o", "-"])
        .arg(app.join("Contents/Info.plist"))
        .output()
        .unwrap();
    assert!(o.status.success(), "{key} missing from Info.plist");
    String::from_utf8(o.stdout).unwrap().trim().to_string()
}

fn version_of(bin: &Path) -> String {
    let o = Command::new(bin).arg("--version").env("LEON_AUTOSTART", "0").output().unwrap();
    assert!(o.status.success(), "{} --version failed", bin.display());
    String::from_utf8(o.stdout).unwrap().trim().to_string()
}

/// Every file under `dir` with its bytes, sorted.
fn snapshot(dir: &Path) -> Vec<(String, Vec<u8>)> {
    let mut out = vec![];
    for e in std::fs::read_dir(dir).unwrap() {
        let p = e.unwrap().path();
        if p.is_dir() {
            out.extend(snapshot(&p));
        } else {
            out.push((p.display().to_string(), std::fs::read(&p).unwrap()));
        }
    }
    out.sort();
    out
}

#[test]
fn bundle_makes_a_valid_app() {
    let tmp = tempfile::tempdir().unwrap();
    let app = tmp.path().join("Leon.app");
    let o = bundle(&app);
    assert!(
        o.status.success(),
        "bundle.sh failed: {}{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    );

    let lint = Command::new("plutil").arg("-lint").arg(app.join("Contents/Info.plist")).output().unwrap();
    assert!(lint.status.success(), "plutil -lint: {}", String::from_utf8_lossy(&lint.stdout));
    assert_eq!(plist(&app, "CFBundleIdentifier"), "dev.leon.app");
    assert_eq!(plist(&app, "CFBundleExecutable"), "leon");
    assert_eq!(plist(&app, "CFBundlePackageType"), "APPL");

    let want = version_of(Path::new(env!("CARGO_BIN_EXE_leon")));
    let v = want.strip_prefix("leon ").expect("--version prints `leon X (commit)`");
    let v = v.split(' ').next().unwrap();
    assert_eq!(plist(&app, "CFBundleShortVersionString"), v);
    assert_eq!(version_of(&app.join("Contents/MacOS/leon")), want);

    let mut icon = plist(&app, "CFBundleIconFile");
    if !icon.ends_with(".icns") {
        icon.push_str(".icns");
    }
    let bytes = std::fs::read(app.join("Contents/Resources").join(&icon)).expect("the icon file exists");
    assert_eq!(&bytes[..4], b"icns", "{icon} is an icns file");
    assert!(bytes.len() > 10_000, "{icon} holds real images ({} bytes)", bytes.len());

    let res = app.join("Contents/Resources");
    assert_eq!(std::fs::read(res.join("LICENSE")).unwrap(), std::fs::read(help_walk::repo().join("LICENSE")).unwrap());
    let notices = std::fs::read_to_string(res.join("THIRD-PARTY-NOTICES.txt")).expect("the notices are bundled");
    for c in ["- alacritty_terminal ", "- gpui ", "- mlua ", "- lua-src ", "Apache License"] {
        assert!(notices.contains(c), "THIRD-PARTY-NOTICES.txt names {c}");
    }

    let again = bundle(&app);
    assert!(again.status.success(), "rebuilding over its own bundle: {}", String::from_utf8_lossy(&again.stderr));
    assert_eq!(plist(&app, "CFBundleIdentifier"), "dev.leon.app");
}

#[test]
fn bundle_refuses_a_foreign_out() {
    let tmp = tempfile::tempdir().unwrap();
    // a plain directory, and an app with another bundle id
    let plain = tmp.path().join("Plain.app");
    std::fs::create_dir_all(&plain).unwrap();
    std::fs::write(plain.join("notes.txt"), "mine").unwrap();
    let other = tmp.path().join("Other.app");
    std::fs::create_dir_all(other.join("Contents")).unwrap();
    std::fs::write(
        other.join("Contents/Info.plist"),
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict><key>CFBundleIdentifier</key><string>com.example.other</string></dict></plist>
"#,
    )
    .unwrap();
    let file = tmp.path().join("File.app");
    std::fs::write(&file, "a file").unwrap();
    for dir in [&plain, &other] {
        let before = snapshot(dir);
        let o = bundle(dir);
        assert!(!o.status.success(), "bundle.sh overwrote {}", dir.display());
        assert!(String::from_utf8_lossy(&o.stderr).contains(&*dir.display().to_string()), "the error names the path");
        assert_eq!(snapshot(dir), before, "{} changed", dir.display());
    }
    assert!(!bundle(&file).status.success(), "bundle.sh overwrote a file");
    assert_eq!(std::fs::read(&file).unwrap(), b"a file");
}

/// Found in review: `--out link/` (a symlink to one of our bundles, with a slash) must not reach
/// through the link and replace the bundle it points at.
#[test]
fn bundle_refuses_a_symlink_out_even_with_a_slash() {
    let tmp = tempfile::tempdir().unwrap();
    let real = tmp.path().join("real.app");
    assert!(bundle(&real).status.success());
    std::fs::write(real.join("sentinel"), "x").unwrap();
    std::os::unix::fs::symlink("real.app", tmp.path().join("link")).unwrap();
    let o = bundle(&tmp.path().join("link/"));
    assert!(!o.status.success(), "bundle.sh went through the link");
    assert!(real.join("sentinel").exists(), "the linked bundle was replaced");
}
