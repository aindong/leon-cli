//! Serve over Tailscale (.agent/plans/2026-11-17-serve-over-tailscale.md), criterion 9: tailnet:// endpoints
//! parse like ssh:// ones, and never connect off the tailnet.

mod support;

use support::Env;

#[test]
fn tailnet_endpoints_are_hosts_and_bad_ones_are_refused() {
    let env = Env::new();
    env.ok(&["hosts", "add", "box", "tailnet://box.example.ts.net:7300", "tailnet://[fd7a:115c:a1e0::5]"]);
    let list = env.json(&["hosts", "list", "--json"]);
    assert_eq!(
        list[0]["endpoints"],
        serde_json::json!(["tailnet://box.example.ts.net:7300", "tailnet://[fd7a:115c:a1e0::5]"])
    );
    for bad in ["tailnet://me@box", "tailnet://-x", "tailnet://a:0", "tailnet://a/b", "tailnet://"] {
        assert_eq!(env.code(&["hosts", "add", "x", bad]), 2, "{bad}");
        assert_eq!(env.code(&["-H", bad, "ls"]), 2, "-H {bad}");
    }
}

#[test]
fn a_tailnet_endpoint_never_connects_off_the_tailnet() {
    let env = Env::new(); // no seam: loopback is not the tailnet
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let ep = format!("tailnet://127.0.0.1:{}", listener.local_addr().unwrap().port());
    let out = env.run(&["-H", &ep, "ls"]);
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(!out.status.success());
    assert!(err.contains("isn't on the tailnet"), "{err}");
    assert!(listener.accept().is_err(), "nothing connected");
}

#[test]
fn a_tailnet_endpoint_must_be_one_of_your_own_untagged_machines() {
    use support::localapi::{whois, Fake};
    let env = Env::new();
    let fake = Fake::new();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let ep = format!("tailnet://127.0.0.1:{}", listener.local_addr().unwrap().port());
    let try_it = |want: &str| {
        let out = env.cmd().env("LEON_TEST_TAILNET", fake.seam()).args(["-H", &ep, "ls"]).output().unwrap();
        let err = String::from_utf8_lossy(&out.stderr).to_string();
        assert!(!out.status.success(), "{err}");
        assert!(err.contains(want), "says {want:?}: {err}");
    };
    fake.set_whois(Ok(whois("stranger@example.com", &[])));
    try_it("stranger@example.com");
    fake.set_whois(Ok(whois("tagged-devices", &["tag:ci"])));
    try_it("tag:ci");
    fake.down();
    try_it("LocalAPI");
    assert!(listener.accept().is_err(), "nothing connected to a machine that wasn't checked");
}
