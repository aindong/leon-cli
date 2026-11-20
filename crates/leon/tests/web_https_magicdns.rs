//! HTTPS for the web page (.agent/plans/2026-11-19-web-https-cert-files.md), criterion 15: the real
//! setup, a `tailscale cert` file for the machine's MagicDNS name, serves at that name.

mod support;

use serde_json::json;
use support::localapi::{status, Fake, Serving, OWNER};
use support::tls::{self, Ca, When};
use support::web::Ws;

#[test]
fn a_cert_for_the_magicdns_name_serves_at_that_name() {
    let ca = Ca::new();
    let dir = tempfile::tempdir().unwrap();
    let (cert, key) = ca.leaf(&["box.example-tailnet.ts.net"], When::Now).write(dir.path());
    let fake = Fake::new();
    let mut st = status(OWNER, &["127.0.0.1"]);
    st["Self"]["DNSName"] = json!("Box.Example-Tailnet.ts.net.");
    fake.set_status(st);
    let seam = fake.seam();
    let s = Serving::begin(fake, true, &tls::settings(&cert, &key), seam);
    let t = s.until("the https URL", |t| t["web"].as_array().is_some_and(|a| !a.is_empty()));
    let host = format!("box.example-tailnet.ts.net:{}", s.port);
    assert_eq!(t["web"], json!([format!("https://{host}/")]), "only the name the cert covers: {t}");
    assert_eq!(t["web_error"], serde_json::Value::Null, "{t}");
    let page = tls::get(s.port, &ca, "/", &host).unwrap();
    assert_eq!(page.status, 200, "{page:?}");
    let at = page.body.find("name=\"leon-token\" content=\"").unwrap() + 27;
    let token = page.body[at..].split('"').next().unwrap().to_string();
    let conn = tls::connect_as(s.port, &ca, "box.example-tailnet.ts.net").unwrap();
    let ws = Ws::upgrade(Box::new(conn), &host, Some(&format!("https://{host}")), Some(&token));
    assert!(ws.is_ok(), "the WebSocket at the name: {:?}", ws.err());
}
