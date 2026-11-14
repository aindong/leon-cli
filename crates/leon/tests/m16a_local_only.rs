//! M16a criterion 12 (.agent/plans/2026-11-15-m16a-remote-ssh.md): commands about this machine
//! refuse -H (exit 2) instead of quietly running here.

mod support;

use support::remote::Remote;

#[test]
fn local_commands_refuse_a_host() {
    let r = Remote::new();
    for args in [&["app"][..], &["server", "run"], &["server", "start"], &["skill", "show"], &["config", "check"]] {
        let out = r.cmd().arg("-H").arg("ssh://box").args(args).output().unwrap();
        let err = String::from_utf8_lossy(&out.stderr);
        assert_eq!(out.status.code(), Some(2), "{args:?}: {err}");
        assert!(err.contains("doesn't take -H"), "{args:?} says why: {err}");
    }
    assert!(r.ssh_calls().is_empty());
    assert!(!r.local.socket().exists());
}
