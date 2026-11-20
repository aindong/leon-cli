//! HTTPS for the web page (.agent/plans/2026-11-19-web-https-cert-files.md), criteria 8–9: renewed
//! files are picked up while serving, a broken renewal keeps the good certificate, and an HTTPS
//! session asks tailscaled only status and whois.

mod support;

use std::time::{Duration, Instant};

use serde_json::json;
use support::localapi::Serving;
use support::tls::{self, Ca, When};
use support::web::{self, Ws};

#[test]
fn renewed_files_are_served_without_a_restart() {
    tls::fake_tailscale();
    let ca = Ca::new();
    let dir = tempfile::tempdir().unwrap();
    let (cert, key) = ca.leaf(&["127.0.0.1"], When::Now).write(dir.path());
    let s = Serving::start(&tls::settings(&cert, &key));
    s.until("the https URL", |t| t["web"].as_array().is_some_and(|a| !a.is_empty()));
    let old = tls::served_cert(s.port, &ca).unwrap();

    std::fs::write(&cert, "not a certificate").unwrap(); // a renewal half done
    let t = s.until("the bad file noticed", |t| t["web_error"].is_string());
    assert!(t["web_error"].as_str().unwrap().contains(" cert --cert-file"), "{t}");
    assert_eq!(tls::served_cert(s.port, &ca).unwrap(), old, "the good certificate is kept meanwhile");

    ca.leaf(&["127.0.0.1"], When::Now).write(dir.path());
    let end = Instant::now() + Duration::from_secs(10);
    loop {
        let now = tls::served_cert(s.port, &ca).unwrap();
        if now != old {
            break;
        }
        assert!(Instant::now() < end, "the renewed certificate wasn't served within 10 s");
        std::thread::sleep(Duration::from_millis(200));
    }
    assert_eq!(s.tailnet()["web_error"], serde_json::Value::Null, "{}", s.tailnet());
    assert!(!tls::tailscale_runs().contains("cert"), "never ran tailscale cert: {}", tls::tailscale_runs());
}

#[test]
fn an_https_session_asks_tailscaled_only_status_and_whois() {
    let ca = Ca::new();
    let dir = tempfile::tempdir().unwrap();
    let (cert, key) = ca.leaf(&["127.0.0.1"], When::Now).write(dir.path());
    let s = Serving::start(&tls::settings(&cert, &key));
    s.until("the https URL", |t| t["web"].as_array().is_some_and(|a| !a.is_empty()));
    let token = tls::token(s.port, &ca);
    let own = format!("https://127.0.0.1:{}", s.port);
    let mut ws =
        Ws::upgrade(Box::new(tls::connect(s.port, &ca).unwrap()), &web::host(s.port), Some(&own), Some(&token))
            .unwrap();
    ws.send(&json!({"id": 1, "method": "session.list"}).to_string());
    ws.until("the reply", |v| v["id"] == 1);
    let _ = web::get(s.port, "/", &web::host(s.port)); // the plain redirect too
    for p in s.fake.paths() {
        let ok = p == "/localapi/v0/status?peers=false" || p.starts_with("/localapi/v0/whois?addr=");
        assert!(ok, "only status and whois: {p}");
    }
}
