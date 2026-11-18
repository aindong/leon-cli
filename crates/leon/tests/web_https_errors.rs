//! HTTPS for the web page (.agent/plans/2026-11-19-web-https-cert-files.md), criteria 4–7: files
//! that are missing, expired, too open or for another name serve nothing and say why, with the
//! `tailscale cert` command to run.

mod support;

use std::os::unix::fs::PermissionsExt;

use serde_json::{json, Value};
use support::localapi::{status, Fake, Serving, OWNER};
use support::tls::{self, Ca, When};
use support::web;

/// A server with these files (`fake`'s status), once it has said what's wrong.
fn broken(fake: Fake, cert: &std::path::Path, key: &std::path::Path) -> (Serving, Value) {
    tls::fake_tailscale();
    let seam = fake.seam();
    let s = Serving::begin(fake, true, &tls::settings(cert, key), seam);
    let t = s.until("a web error", |t| t["web_error"].is_string());
    (s, t)
}

fn assert_says(t: &Value, words: &[&str]) {
    let e = t["web_error"].as_str().unwrap();
    for w in words {
        assert!(e.contains(w), "web_error says {w:?}: {e}");
    }
    assert_eq!(t["web"], json!([]), "no URL while the files are wrong: {t}");
    assert!(!tls::tailscale_runs().contains("cert"), "never ran tailscale cert: {}", tls::tailscale_runs());
}

/// Nothing is served: the handshake fails or no reply comes.
fn assert_no_page(s: &Serving, ca: &Ca) {
    let got = tls::get(s.port, ca, "/", &web::host(s.port));
    assert!(got.is_err(), "no page with bad files: {got:?}");
}

#[test]
fn missing_files_say_the_command() {
    let ca = Ca::new();
    let dir = tempfile::tempdir().unwrap();
    let (cert, key) = (dir.path().join("web.crt"), dir.path().join("web.key"));
    let (s, t) = broken(Fake::new(), &cert, &key);
    let cmd = format!("tailscale cert --cert-file {} --key-file {}", cert.display(), key.display());
    assert_says(&t, &[cert.to_str().unwrap(), &cmd]);
    assert_no_page(&s, &ca);
}

#[test]
fn an_expired_or_future_cert_is_refused() {
    for when in [When::Expired, When::Future] {
        let ca = Ca::new();
        let dir = tempfile::tempdir().unwrap();
        let (cert, key) = ca.leaf(&["127.0.0.1"], when).write(dir.path());
        let (s, t) = broken(Fake::new(), &cert, &key);
        assert_says(&t, &["valid", "tailscale cert --cert-file"]);
        assert_no_page(&s, &ca);
    }
}

#[test]
fn a_key_others_can_read_is_refused() {
    let ca = Ca::new();
    let dir = tempfile::tempdir().unwrap();
    let (cert, key) = ca.leaf(&["127.0.0.1"], When::Now).write(dir.path());
    std::fs::set_permissions(&key, std::fs::Permissions::from_mode(0o644)).unwrap();
    let (s, t) = broken(Fake::new(), &cert, &key);
    assert_says(&t, &["chmod 600", key.to_str().unwrap()]);
    assert_no_page(&s, &ca);
}

#[test]
fn a_cert_for_another_name_is_refused() {
    let ca = Ca::new();
    let dir = tempfile::tempdir().unwrap();
    let (cert, key) = ca.leaf(&["other.example-tailnet.ts.net"], When::Now).write(dir.path());
    let fake = Fake::new();
    let mut st = status(OWNER, &["127.0.0.1"]);
    st["Self"]["DNSName"] = json!("box.example-tailnet.ts.net.");
    fake.set_status(st);
    let (s, t) = broken(fake, &cert, &key);
    assert_says(&t, &["box.example-tailnet.ts.net", "tailscale cert"]);
    assert_no_page(&s, &ca);
}
