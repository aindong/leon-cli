//! Serve over Tailscale (.agent/plans/2026-11-17-serve-over-tailscale.md), criterion 4: a tagged node is
//! refused unless serve_tailscale_tags lists one of its tags.

mod support;

use support::localapi::{whois, Serving};

#[test]
fn a_tagged_node_is_refused() {
    let s = Serving::start("");
    s.fake.set_whois(Ok(whois("tagged-devices", &["tag:ci"])));
    s.assert_refused();
    assert!(s.log().contains("tag:ci"), "{}", s.log());
}

#[test]
fn a_listed_tag_is_let_in_and_no_other() {
    let s = Serving::start(r#"leon.set("serve_tailscale_tags", {"tag:ci"})"#);
    s.fake.set_whois(Ok(whois("tagged-devices", &["tag:web", "tag:ci"])));
    assert_eq!(s.status_over_tcp()["result"]["state"], "running");
    s.fake.set_whois(Ok(whois("tagged-devices", &["tag:web"])));
    s.assert_refused();
}

#[test]
fn a_tagged_machine_does_not_serve() {
    let fake = support::localapi::Fake::new();
    let mut st = support::localapi::status(support::localapi::OWNER, &["127.0.0.1"]);
    st["Self"]["Tags"] = serde_json::json!(["tag:server"]);
    fake.set_status(st);
    let seam = fake.seam();
    let s = Serving::begin(fake, true, "", seam);
    let t = s.until("stopped", |t| t["state"] == "stopped");
    assert!(t["reason"].as_str().unwrap_or("").contains("tagged"), "{t}");
    assert!(s.connect().is_err());
}
