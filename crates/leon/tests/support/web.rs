//! A raw HTTP and WebSocket client for the web endpoint's tests (docs/architecture/web.md): it
//! writes exactly the bytes a test names, so a test can leave out a header or send a bad one.
//! Every read has a deadline.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::{Duration, Instant};

/// RFC 6455's sample key; the server must answer it with `s3pPLMBiTxaQ9kYGzzhZRbK+xOo=`.
pub const KEY: &str = "dGhlIHNhbXBsZSBub25jZQ==";

#[derive(Debug)]
pub struct Reply {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: String,
}

impl Reply {
    /// The first header named `name` (any case).
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
    }
}

/// Send `bytes` to `port` on 127.0.0.1 and read until the close (5 s at most).
pub fn raw(port: u16, bytes: &[u8]) -> Vec<u8> {
    let mut s = TcpStream::connect(("127.0.0.1", port)).expect("the listener accepts");
    s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    let _ = s.write_all(bytes);
    let mut got = Vec::new();
    let mut buf = [0u8; 8192];
    loop {
        match s.read(&mut buf) {
            Ok(0) => return got,
            Ok(n) => got.extend_from_slice(&buf[..n]),
            Err(e) if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => {
                panic!("no close within 5 s; got {:?}", String::from_utf8_lossy(&got))
            }
            Err(_) => return got,
        }
    }
}

/// Parse a whole HTTP reply; None when `bytes` is empty or not HTTP.
pub fn parse(bytes: &[u8]) -> Option<Reply> {
    let text = String::from_utf8_lossy(bytes);
    let (head, body) = text.split_once("\r\n\r\n")?;
    let mut lines = head.split("\r\n");
    let status = lines.next()?.split(' ').nth(1)?.parse().ok()?;
    let headers = lines.filter_map(|l| l.split_once(':')).map(|(k, v)| (k.trim().into(), v.trim().into())).collect();
    Some(Reply { status, headers, body: body.to_string() })
}

/// `GET path` with `Host: host`.
pub fn get(port: u16, path: &str, host: &str) -> Reply {
    let got = raw(port, format!("GET {path} HTTP/1.1\r\nHost: {host}\r\n\r\n").as_bytes());
    parse(&got).unwrap_or_else(|| panic!("not an HTTP reply: {:?}", String::from_utf8_lossy(&got)))
}

/// The page's own Host for the test server.
pub fn host(port: u16) -> String {
    format!("127.0.0.1:{port}")
}

/// The token in the page at `/`.
pub fn token(port: u16) -> String {
    let page = get(port, "/", &host(port));
    assert_eq!(page.status, 200, "{page:?}");
    let at = page.body.find("name=\"leon-token\" content=\"").expect("the page has the token") + 27;
    page.body[at..].split('"').next().unwrap().to_string()
}

/// A stream a WebSocket runs over: plain TCP, or TLS (support/tls.rs).
pub trait Io: Read + Write {
    fn timeout(&self, d: Duration);
}

impl Io for TcpStream {
    fn timeout(&self, d: Duration) {
        self.set_read_timeout(Some(d)).unwrap();
    }
}

impl Io for rustls::StreamOwned<rustls::ClientConnection, TcpStream> {
    fn timeout(&self, d: Duration) {
        self.sock.set_read_timeout(Some(d)).unwrap();
    }
}

pub struct Ws {
    s: Box<dyn Io>,
    buf: Vec<u8>,
}

impl Ws {
    /// Upgrade with these `Origin` and `token` (None leaves each out). Err: the server's reply.
    pub fn open(port: u16, origin: Option<&str>, token: Option<&str>) -> Result<Ws, Reply> {
        let s = TcpStream::connect(("127.0.0.1", port)).expect("the listener accepts");
        Ws::upgrade(Box::new(s), &host(port), origin, token)
    }

    /// `open`, over a stream already connected (TLS, say).
    pub fn upgrade(mut s: Box<dyn Io>, host: &str, origin: Option<&str>, token: Option<&str>) -> Result<Ws, Reply> {
        s.timeout(Duration::from_secs(5));
        let path = match token {
            Some(t) => format!("/ws?token={t}"),
            None => "/ws".into(),
        };
        let origin = origin.map(|o| format!("Origin: {o}\r\n")).unwrap_or_default();
        let req = format!(
            "GET {path} HTTP/1.1\r\nHost: {host}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n\
             Sec-WebSocket-Key: {KEY}\r\nSec-WebSocket-Version: 13\r\n{origin}\r\n"
        );
        s.write_all(req.as_bytes()).unwrap();
        let mut head = Vec::new();
        let mut b = [0u8; 1];
        while !head.ends_with(b"\r\n\r\n") {
            match s.read(&mut b) {
                Ok(1) => head.push(b[0]),
                _ => break,
            }
        }
        let reply = parse(&head).unwrap_or_else(|| panic!("no HTTP head: {:?}", String::from_utf8_lossy(&head)));
        if reply.status != 101 {
            let mut rest = Vec::new();
            let _ = s.read_to_end(&mut rest);
            return Err(Reply { body: String::from_utf8_lossy(&rest).into(), ..reply });
        }
        assert_eq!(reply.header("Sec-WebSocket-Accept"), Some("s3pPLMBiTxaQ9kYGzzhZRbK+xOo="));
        Ok(Ws { s, buf: Vec::new() })
    }

    /// One masked text frame.
    pub fn send(&mut self, text: &str) {
        let mask = [0x11u8, 0x22, 0x33, 0x44];
        let data = text.as_bytes();
        let mut f = vec![0x81u8];
        match data.len() {
            n if n < 126 => f.push(0x80 | n as u8),
            n if n < 65536 => {
                f.push(0x80 | 126);
                f.extend_from_slice(&(n as u16).to_be_bytes());
            }
            n => {
                f.push(0x80 | 127);
                f.extend_from_slice(&(n as u64).to_be_bytes());
            }
        }
        f.extend_from_slice(&mask);
        f.extend(data.iter().enumerate().map(|(i, b)| b ^ mask[i % 4]));
        self.s.write_all(&f).unwrap();
    }

    /// The next text message within `wait`; None on a timeout or the close.
    pub fn recv(&mut self, wait: Duration) -> Option<String> {
        let end = Instant::now() + wait;
        loop {
            if let Some((len, at)) = frame_len(&self.buf) {
                if self.buf.len() >= at + len {
                    let op = self.buf[0] & 0x0f;
                    let data: Vec<u8> = self.buf.drain(..at + len).skip(at).collect();
                    match op {
                        1 => return Some(String::from_utf8(data).expect("text is UTF-8")),
                        8 => return None,
                        _ => continue,
                    }
                }
            }
            let left = end.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return None;
            }
            self.s.timeout(left);
            let mut b = [0u8; 65536];
            match self.s.read(&mut b) {
                Ok(0) | Err(_) => return None,
                Ok(n) => self.buf.extend_from_slice(&b[..n]),
            }
        }
    }

    /// The next message for which `ok` holds, within 10 s.
    pub fn until(&mut self, what: &str, ok: impl Fn(&serde_json::Value) -> bool) -> serde_json::Value {
        let end = Instant::now() + Duration::from_secs(10);
        loop {
            let left = end.saturating_duration_since(Instant::now());
            let m = self.recv(left).unwrap_or_else(|| panic!("no {what} within 10 s"));
            let v: serde_json::Value = serde_json::from_str(&m).expect("each message is one JSON line");
            if ok(&v) {
                return v;
            }
        }
    }
}

/// A server frame's payload length and where the payload starts; None until the header is whole.
fn frame_len(b: &[u8]) -> Option<(usize, usize)> {
    if b.len() < 2 {
        return None;
    }
    assert_eq!(b[1] & 0x80, 0, "server frames are not masked");
    match b[1] & 0x7f {
        126 if b.len() >= 4 => Some((u16::from_be_bytes([b[2], b[3]]) as usize, 4)),
        127 if b.len() >= 10 => Some((u64::from_be_bytes(b[2..10].try_into().unwrap()) as usize, 10)),
        126 | 127 => None,
        n => Some((n as usize, 2)),
    }
}
