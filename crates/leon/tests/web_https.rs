//! HTTPS for the web page (.agent/plans/2026-11-19-web-https-cert-files.md), criteria 1–3: with
//! `serve_web_cert` and `serve_web_key` naming `tailscale cert`'s files, the page is served over
//! TLS on the same port, its WebSocket needs the https Origin, and plain HTTP only redirects.

mod support;

use serde_json::json;
use support::localapi::Serving;
use support::tls::{self, Ca, When};
use support::web::{self, Ws};

/// A server with HTTPS on, for 127.0.0.1, its CA, and the dir holding the files.
fn https() -> (Serving, Ca, tempfile::TempDir) {
    let ca = Ca::new();
    let dir = tempfile::tempdir().unwrap();
    let (cert, key) = ca.leaf(&["127.0.0.1"], When::Now).write(dir.path());
    let s = Serving::start(&tls::settings(&cert, &key));
    s.until("the https URL", |t| t["web"].as_array().is_some_and(|a| !a.is_empty()));
    (s, ca, dir)
}

#[test]
fn the_page_is_served_over_tls() {
    let (s, ca, _dir) = https();
    assert_eq!(s.tailnet()["web"], json!([format!("https://127.0.0.1:{}/", s.port)]));
    assert_eq!(s.tailnet()["web_error"], serde_json::Value::Null, "{}", s.tailnet());
    let page = tls::get(s.port, &ca, "/", &web::host(s.port)).unwrap();
    assert_eq!(page.status, 200, "{page:?}");
    assert!(page.body.contains("leon-token"));
    let csp = page.header("Content-Security-Policy").unwrap();
    assert!(csp.contains(&format!("wss://127.0.0.1:{}", s.port)) && !csp.contains("ws://"), "{csp}");
    let js = tls::get(s.port, &ca, "/app.js", &web::host(s.port)).unwrap();
    assert!(js.body.contains("wss"), "the page opens a wss: socket on an https page");
}

#[test]
fn the_websocket_over_tls_needs_the_https_origin() {
    let (s, ca, _dir) = https();
    let token = tls::token(s.port, &ca);
    let plain = format!("http://127.0.0.1:{}", s.port);
    let refused =
        Ws::upgrade(Box::new(tls::connect(s.port, &ca).unwrap()), &web::host(s.port), Some(&plain), Some(&token));
    assert_eq!(refused.err().map(|r| r.status), Some(403), "an http Origin on an https page");
    let own = format!("https://127.0.0.1:{}", s.port);
    let mut ws =
        Ws::upgrade(Box::new(tls::connect(s.port, &ca).unwrap()), &web::host(s.port), Some(&own), Some(&token))
            .expect("the https page's own Origin and token");
    ws.send(&json!({"id": 1, "method": "server.version"}).to_string());
    assert!(ws.until("server.version", |v| v["id"] == 1)["result"]["api_version"].is_string());
}

#[test]
fn plain_http_only_redirects() {
    let (s, _ca, _dir) = https();
    let r = web::get(s.port, "/?x=1", &web::host(s.port));
    assert_eq!(r.status, 308, "{r:?}");
    assert_eq!(r.header("Location"), Some(&*format!("https://127.0.0.1:{}/?x=1", s.port)));
    assert!(!r.body.contains("leon-token"), "no token over plain HTTP");
    let ws = Ws::open(s.port, Some(&format!("http://127.0.0.1:{}", s.port)), Some("x"));
    assert_eq!(ws.err().map(|r| r.status), Some(308), "no plain WebSocket either");
    assert_eq!(web::get(s.port, "/", &format!("evil.example:{}", s.port)).status, 421);
}
