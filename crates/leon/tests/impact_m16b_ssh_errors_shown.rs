//! M16b impact: the link now pipes ssh's stderr (to keep the last line as the reason a host is
//! unreachable). The CLI must still show ssh's own words.

mod support;

use support::remote::Remote;

#[test]
fn ssh_errors_still_reach_the_terminal() {
    let r = Remote::new();
    let out = r.run(&["-H", "ssh://unreachable-x", "ls"]);
    assert!(!out.status.success());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("ssh: connect to host unreachable-x port 22: Connection refused"), "{err}");
}
