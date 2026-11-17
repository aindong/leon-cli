//! Serve over Tailscale (.agent/plans/2026-11-17-serve-over-tailscale.md), criterion 5 and the brief's
//! "Tailscale off or logged out": a LocalAPI that fails, is slow or is gone refuses, and the
//! listener stops and says why.

mod support;

use std::time::{Duration, Instant};

use support::localapi::Serving;

#[test]
fn an_error_or_a_slow_answer_refuses() {
    let s = Serving::start("");
    for code in [500, 404] {
        s.fake.set_whois(Err(code));
        s.assert_refused();
    }
    assert!(s.log().lines().filter(|l| l.starts_with("leon: tailnet: refused 127.0.0.1:")).count() >= 2, "{}", s.log());
    s.fake.set_whois(Ok(support::localapi::whois(support::localapi::OWNER, &[])));
    s.fake.set_delay(Duration::from_secs(6));
    let t = Instant::now();
    s.assert_turned_away();
    assert!(t.elapsed() < Duration::from_millis(3500), "refused within 3 s: {:?}", t.elapsed());
}

#[test]
fn tailscaled_gone_stops_serving() {
    let s = Serving::start("");
    s.fake.down();
    s.assert_turned_away();
    let t = s.until("stopped", |t| t["state"] == "stopped");
    assert!(t["reason"].as_str().unwrap_or("").contains("LocalAPI"), "{t}");
    assert!(s.connect().is_err(), "the listener is closed");
    assert!(s.log().contains("leon: tailnet: stopped"), "{}", s.log());
}

#[test]
fn logged_out_stops_serving() {
    let s = Serving::start("");
    s.fake.set_status(serde_json::json!({ "BackendState": "NeedsLogin" }));
    let t = s.until("stopped", |t| t["state"] == "stopped");
    assert!(t["reason"].as_str().unwrap_or("").contains("needs a login"), "{t}");
    assert!(s.connect().is_err(), "the listener is closed");
}
