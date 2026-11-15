//! `leon --version` names the build: the workspace version and the commit it was built from.

use std::process::Command;

#[test]
fn version_names_the_commit() {
    let o = Command::new(env!("CARGO_BIN_EXE_leon")).arg("--version").env("LEON_AUTOSTART", "0").output().unwrap();
    assert!(o.status.success());
    let got = String::from_utf8(o.stdout).unwrap().trim().to_string();
    // The tree the test runs in is the one cargo just built, so its HEAD is the commit in the binary.
    // With no git to ask (a worktree's source in Docker), build.rs had none either: the bare version.
    let head = Command::new("git").args(["-C", env!("CARGO_MANIFEST_DIR"), "rev-parse", "--short=12", "HEAD"]).output();
    let want = match head {
        Ok(h) if h.status.success() => {
            format!("leon {} ({})", env!("CARGO_PKG_VERSION"), String::from_utf8_lossy(&h.stdout).trim())
        }
        _ => format!("leon {}", env!("CARGO_PKG_VERSION")),
    };
    assert_eq!(got, want);
}
