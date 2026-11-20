//! The web endpoint (.agent/plans/2026-11-19-web-client-tailnet.md), criteria 7–8: a WebSocket
//! opens only with the page's own Origin *and* the page's token. Each test keeps the other layer
//! right, so each fails if its own layer is gone.

mod support;

use support::localapi::Serving;
use support::web::{self, Ws};

const WEB: &str = "leon.set(\"serve_web\", true)";

#[test]
fn a_missing_or_foreign_origin_is_refused() {
    let s = Serving::start(WEB);
    let token = web::token(s.port);
    let own = format!("http://127.0.0.1:{}", s.port);
    for origin in [None, Some("http://evil.example"), Some("null"), Some(&*format!("https://127.0.0.1:{}", s.port))] {
        match Ws::open(s.port, origin, Some(&token)) {
            Ok(_) => panic!("Origin {origin:?} was let in"),
            Err(r) => assert_eq!(r.status, 403, "Origin {origin:?}: {r:?}"),
        }
    }
    assert!(Ws::open(s.port, Some(&own), Some(&token)).is_ok(), "the page's own Origin and token");
}

#[test]
fn a_missing_or_wrong_token_is_refused() {
    let s = Serving::start(WEB);
    let token = web::token(s.port);
    let own = format!("http://127.0.0.1:{}", s.port);
    let last = if token.ends_with('0') { '1' } else { '0' };
    let wrong = format!("{}{last}", &token[..token.len() - 1]);
    for t in [None, Some(""), Some("x"), Some(&*wrong)] {
        match Ws::open(s.port, Some(&own), t) {
            Ok(_) => panic!("token {t:?} was let in"),
            Err(r) => assert_eq!(r.status, 403, "token {t:?}: {r:?}"),
        }
    }
    assert!(Ws::open(s.port, Some(&own), Some(&token)).is_ok());
}
