//! Serve over Tailscale (.agent/plans/2026-11-17-serve-over-tailscale.md), criterion 6: the owner gets the
//! same API through the same code, each connection checked again; leon reaches it with
//! tailnet:// and no ssh.

mod support;

use std::os::unix::fs::PermissionsExt;

use support::localapi::{whois, Serving};

#[test]
fn the_owner_is_let_in_and_every_connection_is_checked() {
    let s = Serving::start("");
    assert_eq!(s.status_over_tcp()["result"]["state"], "running");
    let asked = s.fake.whois_calls();
    s.fake.set_whois(Ok(whois("stranger@example.com", &[])));
    s.assert_refused();
    assert_eq!(s.fake.whois_calls(), asked + 1, "the same address is asked about again");
}

#[test]
fn leon_reaches_it_over_tailnet_with_no_ssh() {
    let s = Serving::start("");
    let ssh = s.env.dir.path().join("ssh");
    std::fs::write(&ssh, "#!/bin/sh\ntouch \"$0.ran\"\nexit 255\n").unwrap();
    std::fs::set_permissions(&ssh, std::fs::Permissions::from_mode(0o755)).unwrap();
    let ep = format!("tailnet://127.0.0.1:{}", s.port);
    let leon = |args: &[&str]| {
        let out = s.cmd().env("LEON_SSH", &ssh).arg("-H").arg(&ep).args(args).output().unwrap();
        assert!(out.status.success(), "leon -H {ep} {args:?}: {}", String::from_utf8_lossy(&out.stderr));
        String::from_utf8_lossy(&out.stdout).to_string()
    };
    let block = leon(&["run", "--", "sleep", "300"]);
    assert!(block.trim().starts_with("block:"), "{block}");
    assert!(leon(&["ls"]).lines().count() >= 1);
    assert!(leon(&["server", "status"]).starts_with("running"));
    let mut cmd = s.cmd();
    cmd.env("LEON_SSH", &ssh).arg("-H").arg(&ep);
    let events = support::stream(cmd, &[]);
    drop(events);
    assert!(!s.env.dir.path().join("ssh.ran").exists(), "no ssh was run");
}
