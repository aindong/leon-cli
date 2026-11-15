//! Update criterion 17 (.agent/plans/2026-11-16-self-update-core.md): end to end for the CLI. A
//! release made by scripts/release-manifest.sh with a throwaway key, holding a real leon, is
//! served on loopback; a leon labelled 0.0.1 in a temp dir updates itself to it, and the result
//! runs and reports the served version.

mod release_kit;
mod update_kit;

use update_kit::{name, other_build, run, sha256, tarball, Install, Release, OLD, VERSION};

#[test]
fn a_cli_install_updates_itself_to_the_served_release() {
    let rel = Release::unsigned();
    let new = other_build(rel.tmp.path());
    let new_sum = sha256(&std::fs::read(&new).unwrap());
    rel.put(&name("cli"), &tarball(&name("cli"), &new));
    rel.sign();
    let (srv, leon) = (rel.serve(), Install::cli());
    let (before, old_sum) = (leon.state(), sha256(&std::fs::read(&leon.exe).unwrap()));
    assert_ne!(old_sum, new_sum, "the release's leon differs from the installed one");

    let o = leon.update(&rel, &srv, &["--check"]);
    assert_eq!(o.code, 10, "{}", o.text());
    assert!(o.out.contains(&format!("leon {VERSION} is available; you have {OLD}")), "{}", o.text());

    let o = run(leon.cmd(&rel, &srv).arg("self-update"), 600);
    assert_eq!(o.code, 0, "the update goes through: {}", o.text());
    assert!(o.out.contains(&format!("leon {VERSION}")) && o.out.contains(leon.exe.to_str().unwrap()), "{}", o.text());
    assert!(o.out.contains("leon server stop"), "it says what a running server needs: {}", o.text());

    assert_ne!(leon.state().0, before.0, "a new inode: the file was replaced, not written in place");
    assert_eq!(sha256(&std::fs::read(&leon.exe).unwrap()), new_sum, "the installed leon is the release's");
    assert!(leon.leftovers().is_empty(), "no staging directory is left: {:?}", leon.leftovers());
    let version = leon.version();
    assert!(version.starts_with(&format!("leon {VERSION}")) && !version.contains("fake"), "it runs: {version}");
    // the updated leon is a whole leon: it parses its commands and can check again
    let o = run(leon.cmd(&rel, &srv).args(["self-update", "--check"]), 60);
    assert_eq!(o.code, 10, "still labelled {OLD} by the seam, it sees the release again: {}", o.text());
}
