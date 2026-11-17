//! Serve over Tailscale (.agent/plans/2026-11-17-serve-over-tailscale.md), criterion 2: the listener is
//! bound to this machine's Tailscale addresses only, and the port setting can't name an address.

mod support;

use serde_json::json;
use support::localapi::{status, Fake, Serving, OWNER};
use support::Env;

#[test]
fn serves_on_the_tailnet_addresses_only() {
    let s = Serving::start("");
    let t = s.tailnet();
    assert_eq!(t["addresses"], json!([format!("127.0.0.1:{}", s.port)]), "{t}");
    assert_eq!(t["port"], s.port, "{t}");
    assert_eq!(t["login"], OWNER, "{t}");
    assert!(std::net::TcpStream::connect(("::1", s.port)).is_err(), "not on an address the status doesn't list");
}

#[test]
fn addresses_outside_the_tailnet_are_never_bound() {
    let fake = Fake::new();
    fake.set_status(status(OWNER, &["0.0.0.0", "::", "192.168.1.5", "10.0.0.2", "::ffff:100.64.0.1"]));
    let seam = fake.seam();
    let s = Serving::begin(fake, true, "", seam);
    let t = s.until("stopped", |t| t["state"] == "stopped");
    assert!(t["reason"].as_str().unwrap_or("").contains("no Tailscale address"), "{t}");
    assert_eq!(t["addresses"], json!([]), "{t}");
    assert!(s.connect().is_err(), "a bind to 0.0.0.0 would answer on loopback");
}

#[test]
fn loopback_is_the_tailnet_only_when_the_seam_says() {
    let fake = Fake::new(); // its status lists 127.0.0.1
    let seam = json!({ "localapi": fake.socket() }).to_string();
    let s = Serving::begin(fake, true, "", seam);
    let t = s.until("stopped", |t| t["state"] == "stopped");
    assert!(t["reason"].as_str().unwrap_or("").contains("no Tailscale address"), "{t}");
    assert!(s.connect().is_err());
}

#[test]
fn the_port_setting_takes_a_port_and_nothing_else() {
    let env = Env::new();
    let set = |v: &str| {
        let out = env
            .cmd()
            .env("LEON_CONFIG", env.dir.path().join("init.lua"))
            .args(["config", "set", "--", "serve_tailscale_port", v])
            .output()
            .unwrap();
        (out.status.success(), String::from_utf8_lossy(&out.stderr).to_string())
    };
    for bad in ["0.0.0.0:7297", "0", "65536", "7.5", "-1", "::"] {
        let (ok, err) = set(bad);
        assert!(!ok, "{bad} was taken");
        assert!(err.contains("serve_tailscale_port"), "{bad}: {err}");
    }
    assert!(set("8022").0);
}
