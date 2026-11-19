//! Review finding (correctness + skeptic): a Host the page is served on (a trailing dot on the
//! name) must also open the page's WebSocket, or the page reloads forever.

mod support;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;
use support::localapi::Serving;
use support::web;

#[test]
fn a_host_the_page_is_served_on_also_opens_the_websocket() {
    let s = Serving::start("leon.set(\"serve_web\", true)");
    let host = format!("127.0.0.1.:{}", s.port);
    let page = web::get(s.port, "/", &host);
    if page.status != 200 {
        return; // refused outright: no reload loop either
    }
    let at = page.body.find("name=\"leon-token\" content=\"").unwrap() + 27;
    let token = page.body[at..].split('"').next().unwrap().to_string();
    let mut t = TcpStream::connect(("127.0.0.1", s.port)).unwrap();
    t.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    let req = format!(
        "GET /ws?token={token} HTTP/1.1\r\nHost: {host}\r\nOrigin: http://{host}\r\nUpgrade: websocket\r\n\
         Connection: Upgrade\r\nSec-WebSocket-Key: {}\r\nSec-WebSocket-Version: 13\r\n\r\n",
        web::KEY
    );
    t.write_all(req.as_bytes()).unwrap();
    let mut head = Vec::new();
    let mut b = [0u8; 1];
    while !head.ends_with(b"\r\n\r\n") {
        match t.read(&mut b) {
            Ok(1) => head.push(b[0]),
            _ => break,
        }
    }
    let head = String::from_utf8_lossy(&head);
    assert!(head.starts_with("HTTP/1.1 101"), "page served for Host {host} but its WebSocket refused: {head}");
}
