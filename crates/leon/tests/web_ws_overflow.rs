//! Review finding (correctness + security + cross-model): a continuation frame declaring
//! u64::MAX bytes after a 1-byte first part must close with 1009, not panic the connection.

mod support;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;
use support::localapi::Serving;
use support::web;

#[test]
fn a_continuation_declaring_u64_max_closes_with_1009() {
    let s = Serving::start("leon.set(\"serve_web\", true)");
    let token = web::token(s.port);
    let mut t = TcpStream::connect(("127.0.0.1", s.port)).unwrap();
    t.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    let req = format!(
        "GET /ws?token={token} HTTP/1.1\r\nHost: 127.0.0.1:{p}\r\nOrigin: http://127.0.0.1:{p}\r\nUpgrade: websocket\r\n\
         Connection: Upgrade\r\nSec-WebSocket-Key: {k}\r\nSec-WebSocket-Version: 13\r\n\r\n",
        p = s.port,
        k = web::KEY
    );
    t.write_all(req.as_bytes()).unwrap();
    let mut head = Vec::new();
    let mut b = [0u8; 1];
    while !head.ends_with(b"\r\n\r\n") {
        assert_eq!(t.read(&mut b).unwrap(), 1);
        head.push(b[0]);
    }
    assert!(head.starts_with(b"HTTP/1.1 101"), "{}", String::from_utf8_lossy(&head));
    let mut f = vec![0x01, 0x81, 0, 0, 0, 0, b'{']; // first part, masked with zeros, no FIN
    f.extend([0x80, 0x80 | 127]); // FIN continuation, 64-bit length
    f.extend(u64::MAX.to_be_bytes());
    f.extend([0, 0, 0, 0, b'x']);
    t.write_all(&f).unwrap();
    let mut got = Vec::new();
    let mut buf = [0u8; 64];
    loop {
        match t.read(&mut buf) {
            Ok(0) | Err(_) => break,
            Ok(n) => got.extend_from_slice(&buf[..n]),
        }
    }
    assert!(got.windows(4).any(|w| w == [0x88, 2, 0x03, 0xF1]), "no close 1009: {got:?}");
}
