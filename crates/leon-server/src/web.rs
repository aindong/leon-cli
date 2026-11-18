//! The web page over the tailnet (docs/architecture/web.md). `tailnet.rs` admits a connection
//! first; `route` then looks at its first byte: `{` is the socket's NDJSON as before, a capital
//! letter is an HTTP request (served here only with `serve_web` on), anything else is closed.
//! HTTP is a small GET-only subset: the page and its assets (in the binary), and `/ws`, the
//! WebSocket that carries the socket's own API (`websocket.rs`). Every request must name this
//! server's own Host; the WebSocket also needs the page's Origin and the token the page holds.

use std::sync::Arc;
use std::time::Duration;

use leon_core::theme;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use crate::serve::Shared;

/// The longest request head read, and the time it may take.
const MOST_HEAD: usize = 8 << 10;
const HEAD_TIME: Duration = Duration::from_secs(5);
const MOST_HEADERS: usize = 64;

/// The page and its assets, built into the binary: nothing is fetched from anywhere else.
const ASSETS: &[(&str, &str, &str)] = &[
    ("/app.js", "text/javascript; charset=utf-8", include_str!("../web/app.js")),
    ("/conn.js", "text/javascript; charset=utf-8", include_str!("../web/conn.js")),
    ("/grid.js", "text/javascript; charset=utf-8", include_str!("../web/grid.js")),
    ("/app.css", "text/css; charset=utf-8", include_str!("../web/app.css")),
    ("/icon.svg", "image/svg+xml", include_str!("../web/icon.svg")),
];
const PAGE: &str = include_str!("../web/index.html");

/// Who the page is served as while serving: the Hosts it answers to and the token it hands out.
#[derive(PartialEq, Debug)]
pub(crate) struct Site {
    /// Each listening address as `ADDRESS:PORT` (`[v6]:PORT`) and the MagicDNS name with the port,
    /// lower case; on port 80 the port-less forms too.
    hosts: Vec<String>,
    token: String,
    /// The page's URLs, the MagicDNS one first, for `server.status`.
    pub(crate) urls: Vec<String>,
}

impl Site {
    /// `at`: the bound `ADDRESS:PORT`s.
    pub(crate) fn new(at: &[String], dns: Option<&str>, port: u16, token: String) -> Site {
        let named = dns.map(|d| format!("{d}:{port}"));
        let mut hosts: Vec<String> = named.iter().chain(at).cloned().collect();
        let urls = hosts.iter().map(|h| format!("http://{h}/")).collect();
        if port == 80 {
            let bare: Vec<String> = hosts.iter().filter_map(|h| h.strip_suffix(":80").map(str::to_string)).collect();
            hosts.extend(bare); // browsers leave the default port out of Host and Origin
        }
        Site { hosts, token, urls }
    }

    /// `host` normalised (lower case, a trailing dot on the name dropped) when it is one of ours.
    fn own(&self, host: &str) -> Option<String> {
        let h = host.to_ascii_lowercase();
        let h = match h.rsplit_once(':') {
            Some((name, port)) if !port.contains(']') => format!("{}:{port}", name.trim_end_matches('.')),
            _ => h.trim_end_matches('.').to_string(),
        };
        self.hosts.contains(&h).then_some(h)
    }
}

/// A token nobody else can guess: 32 bytes from the kernel, as hex.
pub(crate) fn new_token() -> String {
    let mut b = [0u8; 32];
    std::fs::File::open("/dev/urandom")
        .and_then(|mut f| std::io::Read::read_exact(&mut f, &mut b))
        .expect("/dev/urandom");
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// Serve an admitted tailnet connection by its first byte. `site`: Some while `serve_web` is on.
pub(crate) async fn route(stream: TcpStream, shared: &Arc<Shared>, site: Option<&Site>) {
    let mut first = [0u8; 1];
    if !matches!(stream.peek(&mut first).await, Ok(1)) {
        return;
    }
    match (first[0], site) {
        (b'{', _) => {
            let (read, write) = stream.into_split();
            let _ = crate::serve::connection(read, write, shared).await;
        }
        (b'A'..=b'Z', Some(site)) => http(stream, shared, site).await,
        _ => {} // HTTP with the web off, TLS, anything else: closed unanswered, never read as requests
    }
}

/// A parsed request head.
struct Head {
    method: String,
    target: String,
    headers: Vec<(String, String)>,
}

impl Head {
    fn get(&self, name: &str) -> Vec<&str> {
        self.headers.iter().filter(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str()).collect()
    }

    fn one(&self, name: &str) -> Option<&str> {
        match self.get(name)[..] {
            [v] => Some(v),
            _ => None,
        }
    }
}

/// Read the head (at most `MOST_HEAD` bytes, `HEAD_TIME`); Err: close at once. The bytes after it
/// come back too (a WebSocket client may send its first frame right away).
async fn read_head(stream: &mut TcpStream) -> Result<(Option<Head>, Vec<u8>), ()> {
    let mut buf = Vec::new();
    let end = loop {
        if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break i;
        }
        if buf.len() >= MOST_HEAD {
            return Err(());
        }
        let mut chunk = [0u8; 2048];
        match stream.read(&mut chunk).await {
            Ok(0) | Err(_) => return Err(()),
            Ok(n) => buf.extend_from_slice(&chunk[..n]),
        }
    };
    let rest = buf.split_off(end + 4);
    Ok((parse(&buf[..end]), rest))
}

fn parse(head: &[u8]) -> Option<Head> {
    let text = std::str::from_utf8(head).ok()?;
    let mut lines = text.split("\r\n");
    let mut first = lines.next()?.split(' ');
    let (method, target, version) = (first.next()?, first.next()?, first.next()?);
    if first.next().is_some() || !version.starts_with("HTTP/1.") || !target.starts_with('/') {
        return None;
    }
    let mut headers = Vec::new();
    for l in lines {
        let (k, v) = l.split_once(':')?;
        if headers.len() >= MOST_HEADERS || k.is_empty() || k.contains(' ') {
            return None;
        }
        headers.push((k.to_string(), v.trim().to_string()));
    }
    Some(Head { method: method.into(), target: target.into(), headers })
}

/// One HTTP request, then the close (or the WebSocket).
async fn http(mut stream: TcpStream, shared: &Arc<Shared>, site: &Site) {
    let (head, rest) = match tokio::time::timeout(HEAD_TIME, read_head(&mut stream)).await {
        Ok(Ok(h)) => h,
        _ => return,
    };
    let Some(head) = head else { return reply(&mut stream, 400, None, "text/plain", "bad request").await };
    let Some(host) = head.one("Host").filter(|h| !h.is_empty()) else {
        return reply(&mut stream, 400, None, "text/plain", "one Host header, please").await;
    };
    let Some(host) = site.own(host) else {
        return reply(&mut stream, 421, None, "text/plain", "not this server's name").await;
    };
    if head.method != "GET" {
        return reply(&mut stream, 405, Some(&host), "text/plain", "GET only").await;
    }
    let (path, query) = head.target.split_once('?').unwrap_or((&head.target, ""));
    match path {
        "/" => {
            let page = PAGE.replace("{{token}}", &site.token);
            reply(&mut stream, 200, Some(&host), "text/html; charset=utf-8", &page).await
        }
        "/theme.css" => reply(&mut stream, 200, Some(&host), "text/css; charset=utf-8", &theme_css()).await,
        "/ws" => upgrade(stream, rest, shared, site, &host, &head, query).await,
        _ => match ASSETS.iter().find(|(p, _, _)| *p == path) {
            Some((_, kind, body)) => reply(&mut stream, 200, Some(&host), kind, body).await,
            None => reply(&mut stream, 404, Some(&host), "text/plain", "not found").await,
        },
    }
}

/// The WebSocket: only from the page's own Origin, with the page's token. The token is never
/// logged (it is in the request target).
async fn upgrade(
    mut stream: TcpStream,
    rest: Vec<u8>,
    shared: &Arc<Shared>,
    site: &Site,
    host: &str,
    head: &Head,
    query: &str,
) {
    if head.one("Origin") != Some(&format!("http://{host}")) {
        return reply(&mut stream, 403, Some(host), "text/plain", "not from this page").await;
    }
    let token = query.split('&').find_map(|kv| kv.strip_prefix("token=")).unwrap_or("");
    if !same(token.as_bytes(), site.token.as_bytes()) {
        return reply(&mut stream, 403, Some(host), "text/plain", "reload the page").await;
    }
    let has = |name: &str, word: &str| {
        head.get(name).iter().any(|v| v.split(',').any(|w| w.trim().eq_ignore_ascii_case(word)))
    };
    let key = head.one("Sec-WebSocket-Key").filter(|k| k.len() == 24);
    let Some(key) = key.filter(|_| has("Upgrade", "websocket") && has("Connection", "upgrade")) else {
        return reply(&mut stream, 400, Some(host), "text/plain", "not a WebSocket request").await;
    };
    if head.one("Sec-WebSocket-Version") != Some("13") {
        return reply(&mut stream, 400, Some(host), "text/plain", "WebSocket version 13 only").await;
    }
    let ok = format!(
        "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {}\r\n\r\n",
        crate::websocket::accept_key(key)
    );
    if stream.write_all(ok.as_bytes()).await.is_ok() {
        crate::websocket::bridge(stream, rest, shared).await;
    }
}

/// Equal, in a time that doesn't depend on where they differ.
fn same(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |d, (x, y)| d | (x ^ y)) == 0
}

/// Write one response and close. `host`: the checked Host, for the CSP's WebSocket source.
async fn reply(stream: &mut TcpStream, status: u16, host: Option<&str>, kind: &str, body: &str) {
    let reason = match status {
        200 => "OK",
        400 => "Bad Request",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        _ => "Misdirected Request",
    };
    let ws = host.map(|h| format!(" ws://{h}")).unwrap_or_default();
    let head = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nConnection: close\r\n\
         Cache-Control: no-store\r\nContent-Security-Policy: default-src 'none'; script-src 'self'; style-src 'self'; \
         img-src 'self'; font-src 'self'; connect-src 'self'{ws}; base-uri 'none'; form-action 'none'; \
         frame-ancestors 'none'\r\nX-Frame-Options: DENY\r\nX-Content-Type-Options: nosniff\r\n\
         Referrer-Policy: no-referrer\r\nCross-Origin-Opener-Policy: same-origin\r\n\
         Cross-Origin-Resource-Policy: same-origin\r\n\r\n",
        body.len()
    );
    let _ = stream.write_all(head.as_bytes()).await;
    let _ = stream.write_all(body.as_bytes()).await;
    let _ = stream.shutdown().await;
}

/// leon-dark as CSS variables: the theme `block.watch` resolves screens with unless asked
/// otherwise, so the page and the screens agree.
fn theme_css() -> String {
    let t = theme::DARK;
    let vars = [
        ("bg", t.bg),
        ("fg", t.fg),
        ("cursor", t.cursor),
        ("chrome", t.chrome),
        ("hover", t.hover),
        ("border", t.border),
        ("text", t.text),
        ("title", t.title),
        ("muted", t.muted),
        ("accent", t.accent),
        ("selected", t.selected),
        ("working", t.working),
        ("done", t.done),
        ("blocked", t.blocked),
        ("error", t.error),
    ];
    let body: String = vars.iter().map(|(k, v)| format!("  --{k}: #{v:06x};\n")).collect();
    format!(":root {{\n{body}}}\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_site_answers_to_its_own_names_only() {
        let at = ["100.64.0.5:7297".to_string(), "[fd7a:115c:a1e0::5]:7297".to_string()];
        let s = Site::new(&at, Some("box.example.ts.net"), 7297, "t".into());
        assert_eq!(s.urls[0], "http://box.example.ts.net:7297/");
        for ok in ["100.64.0.5:7297", "[FD7A:115C:A1E0::5]:7297", "Box.Example.ts.net.:7297", "box.example.ts.net:7297"]
        {
            assert!(s.own(ok).is_some(), "{ok}");
        }
        for bad in ["100.64.0.5", "box:7297", "evil.example:7297", "100.64.0.5:72970", "box.example.ts.net"] {
            assert!(s.own(bad).is_none(), "{bad}");
        }
        let s80 = Site::new(&["100.64.0.5:80".to_string()], None, 80, "t".into());
        assert!(s80.own("100.64.0.5").is_some() && s80.own("100.64.0.5:80").is_some());
    }

    #[test]
    fn tokens_compare_whole() {
        assert!(same(b"abc", b"abc"));
        assert!(!same(b"abc", b"abd") && !same(b"abc", b"ab") && !same(b"", b"abc"));
        let (a, b) = (new_token(), new_token());
        assert_eq!(a.len(), 64);
        assert_ne!(a, b);
    }

    #[test]
    fn heads_are_read_strictly() {
        assert!(parse(b"GET / HTTP/1.1\r\nHost: a").is_some());
        for bad in [&b"GET / HTTP/1.1 x"[..], b"GET http://a/ HTTP/1.1", b"GET / SPDY/3", b"GET / HTTP/1.1\r\nno colon"]
        {
            assert!(parse(bad).is_none(), "{}", String::from_utf8_lossy(bad));
        }
        let many: String = (0..=MOST_HEADERS).map(|i| format!("\r\nX-{i}: y")).collect();
        assert!(parse(format!("GET / HTTP/1.1{many}").as_bytes()).is_none());
    }
}
