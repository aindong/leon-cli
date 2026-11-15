//! `scripts/release.sh` checks its arguments before it clones or builds anything. The build itself
//! is proven by running it (docs/audit-0.1.0.md); these are the cheap refusals.

use std::path::PathBuf;
use std::process::Command;

fn release(args: &[&str]) -> std::process::Output {
    let script = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../scripts/release.sh");
    Command::new("bash").arg(script).args(args).output().unwrap()
}

#[test]
fn release_script_refuses_bad_arguments_before_building() {
    let help = release(&["--help"]);
    assert!(help.status.success());
    let text = String::from_utf8_lossy(&help.stdout);
    assert!(text.contains("SHA256SUMS") && text.contains("never tags, pushes or uploads"), "{text}");

    assert_eq!(release(&["--linux", "riscv"]).status.code(), Some(2));
    assert_eq!(release(&["--frobnicate"]).status.code(), Some(2));
    if cfg!(target_os = "macos") {
        let o = release(&["--commit", "no-such-revision-xyz", "--linux", "none"]);
        assert_eq!(o.status.code(), Some(1), "{}", String::from_utf8_lossy(&o.stderr));
        assert!(String::from_utf8_lossy(&o.stderr).contains("is not a commit"));
    }
}
