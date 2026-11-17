//! Serve over Tailscale (.agent/plans/2026-11-17-serve-over-tailscale.md), criterion 3: a peer signed in as
//! someone else, or this machine itself, is closed before any protocol byte, with no reply.

mod support;

use support::localapi::{whois, whois_node, Serving, OWNER, SELF_NODE};

#[test]
fn another_login_is_refused_before_any_protocol_byte() {
    let s = Serving::start("");
    s.fake.set_whois(Ok(whois("stranger@example.com", &[])));
    s.assert_refused();
    let log = s.log();
    let lines: Vec<&str> = log.lines().filter(|l| l.starts_with("leon: tailnet: refused 127.0.0.1:")).collect();
    assert_eq!(lines.len(), 1, "one line per refusal: {log}");
    assert!(lines[0].contains("stranger@example.com"), "with the reason: {}", lines[0]);
    let r = &s.tailnet()["last_refusal"];
    assert!(r["peer"].as_str().unwrap_or("").starts_with("127.0.0.1:"), "{r}");
    assert!(r["reason"].as_str().unwrap_or("").contains("stranger@example.com"), "{r}");
}

#[test]
fn this_machine_itself_is_refused() {
    // another local user dialing this machine's own tailnet address is "the owner" by login
    let s = Serving::start("");
    s.fake.set_whois(Ok(whois_node(SELF_NODE, OWNER, &[])));
    s.assert_refused();
    assert!(s.log().contains("this machine"), "{}", s.log());
}

#[test]
fn a_whois_without_a_login_or_node_is_refused() {
    let s = Serving::start("");
    for v in [
        serde_json::json!({ "Node": { "StableID": "npeer" } }),
        serde_json::json!({ "UserProfile": { "LoginName": OWNER } }),
        serde_json::json!([]),
    ] {
        s.fake.set_whois(Ok(v));
        s.assert_refused();
    }
}
