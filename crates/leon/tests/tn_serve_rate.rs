//! Serve over Tailscale (.agent/plans/2026-11-17-serve-over-tailscale.md), criterion 8: refused connections
//! are rate-limited per address.

mod support;

use support::localapi::{whois, Serving};

#[test]
fn repeated_refusals_from_one_address_are_cut_off() {
    let s = Serving::start("");
    s.fake.set_whois(Ok(whois("stranger@example.com", &[])));
    for _ in 0..5 {
        s.assert_refused();
    }
    let asked = s.fake.whois_calls();
    for _ in 0..3 {
        s.assert_refused();
    }
    assert_eq!(s.fake.whois_calls(), asked, "past the limit tailscaled isn't asked");
    let limited = s.log().lines().filter(|l| l.contains("too many refused connections")).count();
    assert_eq!(limited, 1, "one line when the limit starts: {}", s.log());
}
