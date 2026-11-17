//! Review proof (the security reviewer, on serve over Tailscale): after Tailscale switches
//! account, an unrelated setting change must not start serving the new account's machines.

mod support;

use support::localapi::{status, Serving};

#[test]
fn an_unrelated_setting_change_does_not_adopt_a_new_tailscale_account() {
    let s = Serving::start("");
    s.fake.set_status(status("intruder@example.com", &["127.0.0.1"]));
    s.until("stopped for the switch", |t| t["state"] == "stopped");
    let out = s.cmd().args(["config", "set", "check_for_updates", "false"]).output().unwrap();
    assert!(out.status.success(), "config set: {}", String::from_utf8_lossy(&out.stderr));
    // a negative check: two supervisor ticks (2 s each) for a re-pin to show, if it happened
    std::thread::sleep(std::time::Duration::from_secs(5));
    let t = s.tailnet();
    assert_ne!(t["state"], "serving", "adopted the new account after an unrelated setting change: {t}");
}
