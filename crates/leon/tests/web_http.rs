//! The web endpoint (.agent/plans/2026-11-19-web-client-tailnet.md), criteria 5–6: only the
//! server's own Host is answered (no DNS rebinding), and every page carries a strict CSP and
//! refuses framing, with every asset served from the binary.

mod support;

use support::localapi::Serving;
use support::web;

const WEB: &str = "leon.set(\"serve_web\", true)";

#[test]
fn another_host_gets_no_page_and_no_token() {
    let s = Serving::start(WEB);
    for host in [format!("evil.example:{}", s.port), "127.0.0.1".into(), format!("127.0.0.1:{}0", s.port)] {
        let r = web::get(s.port, "/", &host);
        assert_eq!(r.status, 421, "Host {host:?}: {r:?}");
        assert!(!r.body.contains("leon-token"), "Host {host:?}: no token");
    }
    let r = web::get(s.port, "/", &web::host(s.port));
    assert_eq!(r.status, 200, "the server's own Host: {r:?}");
    let own = web::host(s.port);
    for head in ["GET / HTTP/1.1\r\n\r\n".to_string(), format!("GET / HTTP/1.1\r\nHost: {own}\r\nHost: {own}\r\n\r\n")]
    {
        let r = web::parse(&web::raw(s.port, head.as_bytes())).expect("an HTTP reply");
        assert_eq!(r.status, 400, "{head:?}: {r:?}");
    }
}

#[test]
fn a_rebound_name_cannot_open_the_websocket() {
    let s = Serving::start(WEB);
    let token = web::token(s.port);
    let evil = format!("evil.example:{}", s.port);
    let req = format!(
        "GET /ws?token={token} HTTP/1.1\r\nHost: {evil}\r\nOrigin: http://{evil}\r\nUpgrade: websocket\r\n\
         Connection: Upgrade\r\nSec-WebSocket-Key: {}\r\nSec-WebSocket-Version: 13\r\n\r\n",
        web::KEY
    );
    let r = web::parse(&web::raw(s.port, req.as_bytes())).expect("an HTTP reply");
    assert_eq!(r.status, 421, "the Host is checked before the Origin: {r:?}");
}

#[test]
fn pages_carry_a_strict_policy() {
    let s = Serving::start(WEB);
    let page = web::get(s.port, "/", &web::host(s.port));
    assert_eq!(page.status, 200);
    let csp = page.header("Content-Security-Policy").expect("a CSP");
    for d in ["default-src 'none'", "script-src 'self'", "frame-ancestors 'none'", "base-uri 'none'"] {
        assert!(csp.contains(d), "CSP has {d}: {csp}");
    }
    assert!(!csp.contains("unsafe-inline") && !csp.contains("unsafe-eval"), "{csp}");
    assert_eq!(page.header("X-Frame-Options"), Some("DENY"));
    assert_eq!(page.header("X-Content-Type-Options"), Some("nosniff"));
    assert_eq!(page.header("Cache-Control"), Some("no-store"), "the page holds the token");
    assert!(page.header("Access-Control-Allow-Origin").is_none(), "no CORS");
    assert!(!page.body.contains("http://") && !page.body.contains("https://"), "nothing from elsewhere");
    for (path, kind) in [
        ("/app.js", "text/javascript"),
        ("/conn.js", "text/javascript"),
        ("/grid.js", "text/javascript"),
        ("/app.css", "text/css"),
        ("/theme.css", "text/css"),
        ("/icon.svg", "image/svg+xml"),
    ] {
        let r = web::get(s.port, path, &web::host(s.port));
        assert_eq!(r.status, 200, "{path}");
        assert!(r.header("Content-Type").is_some_and(|t| t.starts_with(kind)), "{path}: {r:?}");
        assert!(r.header("Content-Security-Policy").is_some(), "{path} has the CSP too");
        let body = r.body.replace("xmlns=\"http://www.w3.org/2000/svg\"", ""); // a namespace's name, not a load
        assert!(!body.contains("https://") && !body.contains("http://"), "{path} loads nothing from elsewhere");
    }
    assert_eq!(web::get(s.port, "/nope", &web::host(s.port)).status, 404);
    let post = web::raw(
        s.port,
        format!("POST / HTTP/1.1\r\nHost: {}\r\nContent-Length: 0\r\n\r\n", web::host(s.port)).as_bytes(),
    );
    assert_eq!(web::parse(&post).map(|r| r.status), Some(405));
}
