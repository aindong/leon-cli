//! HTTPS for the web page (.agent/plans/2026-11-19-web-https-cert-files.md), criteria 14 and 16:
//! a renewal that doesn't fit keeps the good certificate, expiry is noticed while serving, and
//! leon never runs `tailscale cert`.

mod support;

use std::time::Duration;

use serde_json::json;
use support::localapi::Serving;
use support::tls::{self, Ca, When};
use support::web;

fn serving(ca: &Ca, when: When, dir: &std::path::Path) -> Serving {
    tls::fake_tailscale();
    let (cert, key) = ca.leaf(&["127.0.0.1"], when).write(dir);
    let s = Serving::start(&tls::settings(&cert, &key));
    s.until("the https URL", |t| t["web"].as_array().is_some_and(|a| !a.is_empty()));
    s
}

#[test]
fn a_key_that_does_not_match_keeps_the_good_certificate() {
    let ca = Ca::new();
    let dir = tempfile::tempdir().unwrap();
    let s = serving(&ca, When::Now, dir.path());
    let old = tls::served_cert(s.port, &ca).unwrap();
    let other = ca.leaf(&["127.0.0.1"], When::Now);
    std::fs::write(dir.path().join("web.key"), other.key).unwrap(); // the cert stays: they no longer match
    let t = s.until("the mismatch noticed", |t| t["web_error"].is_string());
    assert!(t["web_error"].as_str().unwrap().contains("match"), "{t}");
    assert_eq!(tls::served_cert(s.port, &ca).unwrap(), old, "the good certificate is kept");
    assert!(!tls::tailscale_runs().contains("cert"), "never ran tailscale cert: {}", tls::tailscale_runs());
}

#[test]
fn expiry_is_noticed_while_serving() {
    let ca = Ca::new();
    let dir = tempfile::tempdir().unwrap();
    let s = serving(&ca, When::For(5), dir.path());
    assert!(tls::get(s.port, &ca, "/", &web::host(s.port)).is_ok_and(|r| r.status == 200));
    std::thread::sleep(Duration::from_secs(5)); // the certificate's own clock
    let t = s.until("the expiry noticed", |t| t["web_error"].is_string());
    assert!(t["web_error"].as_str().unwrap().contains("valid"), "{t}");
    assert_eq!(t["web"], json!([]), "{t}");
    assert!(tls::get(s.port, &ca, "/", &web::host(s.port)).is_err(), "no page with an expired certificate");
    assert!(!tls::tailscale_runs().contains("cert"), "never ran tailscale cert: {}", tls::tailscale_runs());
}
