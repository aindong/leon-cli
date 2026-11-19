//! The web endpoint (.agent/plans/2026-11-19-web-client-tailnet.md), criteria 14–15: a whole web
//! session asks tailscaled only `status` and `whois` (never a cert, serve or prefs call), and
//! `serve_web` alone opens nothing.

mod support;

use serde_json::json;
use std::time::{Duration, Instant};

use support::localapi::{free_port, Fake, Serving};
use support::web::{self, Ws};

#[test]
fn a_web_session_asks_tailscaled_only_status_and_whois() {
    let s = Serving::start("leon.set(\"serve_web\", true)");
    let token = web::token(s.port);
    for path in ["/app.js", "/app.css", "/theme.css"] {
        assert_eq!(web::get(s.port, path, &web::host(s.port)).status, 200, "{path}");
    }
    let mut ws = Ws::open(s.port, Some(&format!("http://127.0.0.1:{}", s.port)), Some(&token)).expect("let in");
    ws.send(&json!({"id": 1, "method": "session.list"}).to_string());
    ws.until("the reply", |v| v["id"] == 1);
    let paths = s.fake.paths();
    assert!(paths.iter().any(|p| p.starts_with("/localapi/v0/whois")), "{paths:?}");
    for p in &paths {
        let ok = p == "/localapi/v0/status?peers=false" || p.starts_with("/localapi/v0/whois?addr=");
        assert!(ok, "only status and whois: {p} ({paths:?})");
    }
}

#[test]
fn serve_web_alone_opens_nothing() {
    let fake = Fake::new();
    let seam = fake.seam();
    let port = free_port();
    let extra = format!("leon.set(\"serve_web\", true)\nleon.set(\"serve_tailscale_port\", {port})");
    let s = Serving::begin(fake, false, &extra, seam);
    let settings = s.cmd().args(["api", "call", "server.settings"]).output().unwrap();
    let v: serde_json::Value = serde_json::from_slice(&settings.stdout).unwrap();
    assert_eq!(v["settings"]["serve_web"], true, "the setting took: {v}");
    // two of the supervisor's 2 s looks: nothing may listen at any point
    let end = Instant::now() + Duration::from_secs(5);
    while Instant::now() < end {
        assert!(std::net::TcpStream::connect(("127.0.0.1", port)).is_err(), "serve_web alone listened");
        let st = s.tailnet();
        assert_eq!((st["state"].as_str(), &st["web"]), (Some("off"), &json!([])), "{st}");
        std::thread::sleep(Duration::from_millis(200));
    }
    assert_eq!(s.fake.calls(), 0, "tailscaled was never asked");
}
