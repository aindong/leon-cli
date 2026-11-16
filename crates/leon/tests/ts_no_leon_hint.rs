//! Tailscale criterion 6 (.agent/plans/2026-11-16-tailscale-hosts.md): a host without leon says
//! how to install it there, not ssh's raw words.

mod support;

use support::remote::Remote;

#[test]
fn a_host_without_leon_says_how_to_install_it() {
    let mut r = Remote::new();
    r.path = "/usr/bin:/bin".into(); // no leon on the host
    let out = r.run(&["-H", "ssh://box", "ls"]);
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(!out.status.success());
    assert!(err.contains("install leon on box"), "{err}");
    assert!(err.contains("brew install aindong/leon/leon"), "{err}");
    assert!(err.contains("release tarball"), "or a release tarball: {err}");
}
