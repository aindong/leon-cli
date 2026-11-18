//! Review finding (security, cross-model): a FIFO at serve_web_key must give a web_error, not hang
//! the supervisor.
mod support;

use support::localapi::{Fake, Serving};
use support::tls::{self, Ca, When};

#[test]
fn a_fifo_at_the_key_path_says_why_and_does_not_hang() {
    let ca = Ca::new();
    let dir = tempfile::tempdir().unwrap();
    let (cert, key) = ca.leaf(&["127.0.0.1"], When::Now).write(dir.path());
    std::fs::remove_file(&key).unwrap();
    let made = std::process::Command::new("mkfifo").arg("-m").arg("600").arg(&key).status().unwrap();
    assert!(made.success(), "mkfifo");
    tls::fake_tailscale();
    let fake = Fake::new();
    let seam = fake.seam();
    let s = Serving::begin(fake, true, &tls::settings(&cert, &key), seam);
    let t = s.until("a web error", |t| t["web_error"].is_string());
    assert!(t["web_error"].as_str().unwrap().contains(key.to_str().unwrap()), "{t}");
}
