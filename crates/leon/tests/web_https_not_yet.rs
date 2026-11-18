//! Review finding (correctness): a certificate loaded before its notBefore is served once that
//! time passes, without the files changing.
mod support;
use support::localapi::Serving;
use support::tls::{self, Ca, When};

#[test]
fn a_cert_that_becomes_valid_is_served_once_it_is() {
    let ca = Ca::new();
    let dir = tempfile::tempdir().unwrap();
    let (cert, key) = ca.leaf(&["127.0.0.1"], When::From(7)).write(dir.path());
    let s = Serving::start(&tls::settings(&cert, &key));
    s.until("not valid yet", |t| t["web_error"].is_string());
    std::thread::sleep(std::time::Duration::from_secs(7)); // the certificate's own clock
    s.until("served once valid", |t| t["web"].as_array().is_some_and(|a| !a.is_empty()));
}
