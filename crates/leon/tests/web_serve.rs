//! The web endpoint (.agent/plans/2026-11-19-web-client-tailnet.md), criteria 1–4: it is served
//! only by the tailnet listener, only with `serve_web` on, only to callers `admit` lets in, and a
//! connection that starts like HTTP never reaches the NDJSON reader.

mod support;

use serde_json::json;
use support::localapi::{status, whois, Fake, Serving, OWNER};
use support::web;

const WEB: &str = "leon.set(\"serve_web\", true)";

#[test]
fn web_is_never_served_off_the_tailnet() {
    let fake = Fake::new();
    fake.set_status(status(OWNER, &["0.0.0.0", "::", "192.168.1.5", "10.0.0.2"]));
    let seam = fake.seam();
    let s = Serving::begin(fake, true, WEB, seam);
    let t = s.until("stopped", |t| t["state"] == "stopped");
    assert_eq!(t["web"], json!([]), "no web URL while nothing serves: {t}");
    assert!(s.connect().is_err(), "a bind to 0.0.0.0 would answer on loopback");
}

#[test]
fn web_is_served_on_the_tailnet_address_when_on() {
    let s = Serving::start(WEB);
    let t = s.until("the web URL", |t| t["web"].as_array().is_some_and(|a| !a.is_empty()));
    assert_eq!(t["web"], json!([format!("http://127.0.0.1:{}/", s.port)]), "{t}");
    let page = web::get(s.port, "/", &web::host(s.port));
    assert_eq!(page.status, 200, "{page:?}");
    assert!(page.body.contains("leon-token"), "the page: {}", page.body);
}

#[test]
fn with_web_off_http_is_closed_unanswered() {
    let s = Serving::start("");
    let got = web::raw(s.port, format!("GET / HTTP/1.1\r\nHost: 127.0.0.1:{}\r\n\r\n", s.port).as_bytes());
    assert!(got.is_empty(), "web is off: {:?}", String::from_utf8_lossy(&got));
    let settings = s.cmd().args(["api", "call", "server.settings"]).output().unwrap();
    let v: serde_json::Value = serde_json::from_slice(&settings.stdout).unwrap();
    assert_eq!(v["settings"]["serve_web"], false, "{v}");
    assert_eq!(s.tailnet()["web"], json!([]));
    assert_eq!(s.status_over_tcp()["result"]["state"], "running", "NDJSON callers still get in");
}

#[test]
fn a_refused_caller_gets_no_page() {
    let s = Serving::start(WEB);
    assert_eq!(web::get(s.port, "/", &web::host(s.port)).status, 200, "the owner's machine is let in");
    s.fake.set_whois(Ok(whois("stranger@example.com", &[])));
    let asked = s.fake.whois_calls();
    let got = web::raw(s.port, format!("GET / HTTP/1.1\r\nHost: 127.0.0.1:{}\r\n\r\n", s.port).as_bytes());
    assert!(got.is_empty(), "refused before a byte is read: {:?}", String::from_utf8_lossy(&got));
    assert!(s.fake.whois_calls() > asked, "whois was asked about the caller");
    assert!(s.log().contains("refused"), "logged: {}", s.log());
}

#[test]
fn an_http_body_never_runs_as_a_request() {
    for extra in ["", WEB] {
        let s = Serving::start(extra);
        let body = "{\"id\":1,\"method\":\"server.stop\"}\n";
        let req = format!(
            "POST / HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nContent-Type: text/plain\r\nContent-Length: {}\r\n\r\n{body}",
            s.port,
            body.len()
        );
        let got = web::raw(s.port, req.as_bytes());
        assert!(!String::from_utf8_lossy(&got).contains("\"id\":1"), "no reply to the body ({extra:?})");
        assert!(s.env.socket().exists(), "the server.stop in the body never ran ({extra:?})");
        assert_eq!(s.status_over_tcp()["result"]["state"], "running");
    }
}
