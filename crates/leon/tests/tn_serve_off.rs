//! Serve over Tailscale (.agent/plans/2026-11-17-serve-over-tailscale.md), criterion 1: off by default,
//! nothing listens on TCP and tailscaled is never asked.

mod support;

use support::localapi::{Fake, Serving};

#[test]
fn off_by_default_listens_nowhere_and_asks_nothing() {
    let fake = Fake::new();
    let seam = fake.seam();
    let s = Serving::begin(fake, false, "", seam);
    let t = s.tailnet();
    assert_eq!(t["state"], "off", "{t}");
    assert_eq!(t["addresses"], serde_json::json!([]), "{t}");
    // a negative check: give a serving server's first look at the status (at start) time to happen
    std::thread::sleep(std::time::Duration::from_millis(1500));
    assert_eq!(s.fake.calls(), 0, "the LocalAPI is never asked while the setting is off");
    assert!(s.connect().is_err(), "nothing listens on the port");
}
