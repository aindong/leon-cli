//! HTTPS for the web page (.agent/plans/2026-11-19-web-https-cert-files.md), criteria 13 and 17:
//! with HTTPS set, plain HTTP never serves the page or a token, whatever state the files are in;
//! and a key leon can't read gets advice that works.

mod support;

use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use support::localapi::{Fake, Serving};
use support::tls::{self, Ca, When};
use support::web::{self, Ws};

/// A server with these Lua lines, once it has said what's wrong with HTTPS.
fn broken(lua: &str) -> Serving {
    let fake = Fake::new();
    let seam = fake.seam();
    let s = Serving::begin(fake, true, lua, seam);
    s.until("a web error", |t| t["web_error"].is_string());
    s
}

/// Plain HTTP gets no page, no token and no WebSocket.
fn assert_plain_refused(s: &Serving) {
    let page = web::raw(s.port, format!("GET / HTTP/1.1\r\nHost: {}\r\n\r\n", web::host(s.port)).as_bytes());
    let page = String::from_utf8_lossy(&page);
    assert!(!page.contains("leon-token"), "no token over plain HTTP: {page}");
    assert!(page.is_empty() || page.starts_with("HTTP/1.1 308"), "a redirect or nothing: {page}");
    let ws = Ws::open(s.port, Some(&format!("https://127.0.0.1:{}", s.port)), Some("x"));
    assert!(ws.is_err(), "no plain WebSocket");
}

#[test]
fn plain_http_serves_nothing_while_https_is_set_but_broken() {
    let ca = Ca::new();
    let dir = tempfile::tempdir().unwrap();
    let missing = (dir.path().join("none.crt"), dir.path().join("none.key"));
    assert_plain_refused(&broken(&tls::settings(&missing.0, &missing.1)));

    let (cert, key) = ca.leaf(&["127.0.0.1"], When::Expired).write(dir.path());
    assert_plain_refused(&broken(&tls::settings(&cert, &key)));

    let open = tempfile::tempdir().unwrap();
    let (cert, key) = ca.leaf(&["127.0.0.1"], When::Now).write(open.path());
    std::fs::set_permissions(&key, std::fs::Permissions::from_mode(0o644)).unwrap();
    assert_plain_refused(&broken(&tls::settings(&cert, &key)));

    let s =
        broken(&format!("leon.set(\"serve_web\", true)\nleon.set(\"serve_web_cert\", {:?})", cert.to_str().unwrap()));
    assert!(s.tailnet()["web_error"].as_str().unwrap().contains("serve_web_key"), "{}", s.tailnet());
    assert_plain_refused(&s);
}

#[test]
fn a_key_leon_cannot_read_gets_chown_advice() {
    let ca = Ca::new();
    let dir = tempfile::tempdir().unwrap();
    let (cert, key) = ca.leaf(&["127.0.0.1"], When::Now).write(dir.path());
    std::fs::set_permissions(&key, std::fs::Permissions::from_mode(0o000)).unwrap();
    let s = broken(&tls::settings(&cert, &key));
    let e = s.tailnet()["web_error"].as_str().unwrap().to_string();
    assert!(e.contains("chown") && e.contains(key.to_str().unwrap()), "{e}");
    std::fs::set_permissions(&key, std::fs::Permissions::from_mode(0o600)).unwrap();
}

#[test]
fn the_command_given_works_for_a_user_on_linux() {
    let dir = tempfile::tempdir().unwrap();
    let missing = (dir.path().join("web.crt"), dir.path().join("web.key"));
    let s = broken(&tls::settings(&missing.0, &missing.1));
    let e = s.tailnet()["web_error"].as_str().unwrap().to_string();
    if cfg!(target_os = "linux") {
        assert!(e.contains("sudo tailscale cert") && e.contains("sudo chown"), "{e}");
    } else {
        assert!(e.contains("tailscale cert --cert-file"), "{e}");
    }
    let doc = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/web.md")).unwrap();
    assert!(doc.contains("sudo chown") && doc.contains("--operator"), "docs/web.md gives the Linux steps");
}
