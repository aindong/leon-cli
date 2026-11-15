//! `leon --version` names the build: the workspace version and the commit it was built from.

use std::process::Command;

fn run(cmd: &str, args: &[&str]) -> String {
    let o = Command::new(cmd).args(args).env("LEON_AUTOSTART", "0").output().unwrap();
    assert!(o.status.success(), "{cmd} {args:?}: {}", String::from_utf8_lossy(&o.stderr));
    String::from_utf8(o.stdout).unwrap().trim().to_string()
}

#[test]
fn version_names_the_commit() {
    let got = run(env!("CARGO_BIN_EXE_leon"), &["--version"]);
    let dir = env!("CARGO_MANIFEST_DIR");
    // The tree the test runs in is the one cargo just built, so its HEAD is the commit in the binary.
    let head = run("git", &["-C", dir, "rev-parse", "--short=12", "HEAD"]);
    assert_eq!(got, format!("leon {} ({head})", env!("CARGO_PKG_VERSION")));
    assert_eq!(env!("CARGO_PKG_VERSION"), "0.1.0");
}
