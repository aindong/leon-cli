//! Serve over Tailscale (.agent/plans/2026-11-17-serve-over-tailscale.md), criterion 7: Tailscale switched to
//! another account stops serving until serve_tailscale is turned off and on again.

mod support;

use support::localapi::{status, whois, Serving};

#[test]
fn another_account_stops_serving_until_turned_off_and_on() {
    let s = Serving::start("");
    let mut before = s.admitted();
    let other = "other@example.com";
    s.fake.set_status(status(other, &["127.0.0.1"]));
    s.fake.set_whois(Ok(whois(other, &[])));
    s.assert_turned_away();
    let t = s.until("stopped", |t| t["state"] == "stopped");
    assert!(t["reason"].as_str().unwrap_or("").contains(other), "the reason names the account: {t}");
    assert!(s.connect().is_err(), "the listener is closed");
    before.set_read_timeout(Some(std::time::Duration::from_secs(10))).unwrap();
    let mut rest = Vec::new();
    let closed = std::io::Read::read_to_end(&mut before, &mut rest);
    assert!(
        closed.is_ok() || closed.unwrap_err().kind() == std::io::ErrorKind::ConnectionReset,
        "a connection let in before the switch is closed"
    );
    for on in ["false", "true"] {
        let out = s.cmd().args(["config", "set", "serve_tailscale", on]).output().unwrap();
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    }
    let t = s.until("serving again", |t| t["state"] == "serving");
    assert_eq!(t["login"], other, "{t}");
    assert_eq!(s.status_over_tcp()["result"]["state"], "running");
}
