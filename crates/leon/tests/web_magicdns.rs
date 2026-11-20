//! The web endpoint (.agent/plans/2026-11-19-web-client-tailnet.md), criterion 16: the page answers
//! at the machine's full MagicDNS name (any case), not its short name, and building it needs no node.

mod support;

use serde_json::json;
use support::localapi::{status, Fake, Serving, OWNER};
use support::web;

#[test]
fn the_page_answers_at_the_magicdns_name() {
    let fake = Fake::new();
    let mut st = status(OWNER, &["127.0.0.1"]);
    st["Self"]["DNSName"] = json!("Box.Example-Tailnet.ts.net.");
    fake.set_status(st);
    let seam = fake.seam();
    let s = Serving::begin(fake, true, "leon.set(\"serve_web\", true)", seam);
    let t = s.until("the web URLs", |t| t["web"].as_array().is_some_and(|a| a.len() == 2));
    let name = format!("box.example-tailnet.ts.net:{}", s.port);
    assert_eq!(t["web"], json!([format!("http://{name}/"), format!("http://127.0.0.1:{}/", s.port)]), "{t}");
    for host in [name.clone(), name.to_uppercase(), format!("box.example-tailnet.ts.net.:{}", s.port)] {
        assert_eq!(web::get(s.port, "/", &host).status, 200, "Host {host}");
    }
    assert_eq!(web::get(s.port, "/", &format!("box:{}", s.port)).status, 421, "not the short name");
}

#[test]
fn building_the_page_needs_no_node() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../leon-server");
    assert!(root.join("web/index.html").exists(), "the page is checked in");
    assert!(!root.join("build.rs").exists(), "no build step");
    assert!(!root.join("web/package.json").exists(), "no npm");
}
