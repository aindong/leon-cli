//! The web page over the tailnet (docs/architecture/web.md). `tailnet.rs` admits a connection
//! first; `route` then looks at its first byte: `{` is the socket's NDJSON as before, a capital
//! letter is an HTTP request (served here only with `serve_web` on), anything else is closed.
//! HTTP is a small GET-only subset: the page and its assets (in the binary), and `/ws`, the
//! WebSocket that carries the socket's own API (`websocket.rs`). Every request must name this
//! server's own Host; the WebSocket also needs the page's Origin and the token the page holds.

use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::TcpStream;

use crate::serve::Shared;
use crate::web_tls::Tls;

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

/// Who the page is served as while serving: the Hosts it answers to, the token it hands out, and
/// HTTPS when the settings name `tailscale cert`'s files.
pub(crate) struct Site {
    /// Each listening address as `ADDRESS:PORT` (`[v6]:PORT`) and the MagicDNS name with the port,
    /// lower case; on port 80 the port-less forms too.
    hosts: Vec<String>,
    token: String,
    /// The page's plain URLs, the MagicDNS one first.
    urls: Vec<String>,
    port: u16,
    pub(crate) tls: Option<Tls>,
}

impl Site {
    /// `at`: the bound `ADDRESS:PORT`s.
    pub(crate) fn new(at: &[String], dns: Option<&str>, port: u16, token: String, tls: Option<Tls>) -> Site {
        let named = dns.map(|d| format!("{d}:{port}"));
        let mut hosts: Vec<String> = named.iter().chain(at).cloned().collect();
        let urls = hosts.iter().map(|h| format!("http://{h}/")).collect();
        if port == 80 {
            let bare: Vec<String> = hosts.iter().filter_map(|h| h.strip_suffix(":80").map(str::to_string)).collect();
            hosts.extend(bare); // browsers leave the default port out of Host and Origin
        }
        Site { hosts, token, urls, port, tls }
    }

    /// The page's URLs for `server.status`, and why HTTPS serves nothing: with HTTPS, only the
    /// hosts the certificate covers. Looks at the certificate files (`Tls::refresh`).
    pub(crate) fn status(&self) -> (Vec<String>, Option<String>) {
        let Some(tls) = &self.tls else { return (self.urls.clone(), None) };
        let now = tls.refresh();
        let host = |n: &String| if n.contains(':') { format!("[{n}]") } else { n.clone() };
        (now.names.iter().map(|n| format!("https://{}:{}/", host(n), self.port)).collect(), now.error)
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
        (b'A'..=b'Z', Some(site)) => http(stream, shared, site, false).await,
        (0x16, Some(Site { tls: Some(tls), .. })) => {
            // a TLS handshake: served only with a valid certificate loaded, else closed
            let Some(config) = tls.config() else { return };
            let accept = tokio_rustls::TlsAcceptor::from(config).accept(stream);
            if let Ok(Ok(stream)) = tokio::time::timeout(HEAD_TIME, accept).await {
                http(stream, shared, site.expect("matched"), true).await
            }
        }
        _ => {} // HTTP with the web off, TLS without HTTPS, anything else: closed unanswered
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

/// Read the head (at most about `MOST_HEAD` bytes; the caller gives it `HEAD_TIME`); Err: close at
/// once. The bytes read after it come back too, for the WebSocket that may follow.
async fn read_head(stream: &mut (impl AsyncRead + Unpin)) -> Result<(Option<Head>, Vec<u8>), ()> {
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
        let token = |b: u8| b.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&b);
        if headers.len() >= MOST_HEADERS || k.is_empty() || !k.bytes().all(token) {
            return None;
        }
        headers.push((k.to_string(), v.trim().to_string()));
    }
    Some(Head { method: method.into(), target: target.into(), headers })
}

/// One HTTP request, then the close (or the WebSocket). `secure`: it came over TLS.
async fn http<S>(mut stream: S, shared: &Arc<Shared>, site: &Site, secure: bool)
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let (head, rest) = match tokio::time::timeout(HEAD_TIME, read_head(&mut stream)).await {
        Ok(Ok(h)) => h,
        _ => return,
    };
    let Some(head) = head else { return reply(&mut stream, 400, None, "text/plain", "bad request", secure, "").await };
    let Some(host) = head.one("Host").filter(|h| !h.is_empty()) else {
        return reply(&mut stream, 400, None, "text/plain", "one Host header, please", secure, "").await;
    };
    let Some(host) = site.own(host) else {
        return reply(&mut stream, 421, None, "text/plain", "not this server's name", secure, "").await;
    };
    if !secure && site.tls.is_some() {
        // HTTPS is set: plain HTTP never gets the page, its token or the WebSocket, whatever state
        // the certificate is in
        let to = format!("Location: https://{host}{}\r\n", head.target);
        return reply(&mut stream, 308, Some(&host), "text/plain", "use https", secure, &to).await;
    }
    if head.method != "GET" {
        return reply(&mut stream, 405, Some(&host), "text/plain", "GET only", secure, "").await;
    }
    let path = head.target.split_once('?').map_or(head.target.as_str(), |(p, _)| p);
    match path {
        "/" => {
            let page = PAGE.replace("{{token}}", &site.token);
            reply(&mut stream, 200, Some(&host), "text/html; charset=utf-8", &page, secure, "").await
        }
        "/theme.css" => reply(&mut stream, 200, Some(&host), "text/css; charset=utf-8", &theme_css(), secure, "").await,
        "/ws" => upgrade(stream, rest, shared, site, &host, &head, secure).await,
        _ => match ASSETS.iter().find(|(p, _, _)| *p == path) {
            Some((_, kind, body)) => reply(&mut stream, 200, Some(&host), kind, body, secure, "").await,
            None => reply(&mut stream, 404, Some(&host), "text/plain", "not found", secure, "").await,
        },
    }
}

/// The WebSocket: only from the page's own Origin, with the page's token. The token is never
/// logged (it is in the request target).
async fn upgrade<S>(
    mut stream: S,
    rest: Vec<u8>,
    shared: &Arc<Shared>,
    site: &Site,
    host: &str,
    head: &Head,
    secure: bool,
) where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    // the Origin's host, normalised as the Host was: a trailing dot on the name is the same page
    let scheme = if secure { "https://" } else { "http://" };
    let origin = head.one("Origin").and_then(|o| o.strip_prefix(scheme)).and_then(|h| site.own(h));
    if origin.as_deref() != Some(host) {
        return reply(&mut stream, 403, Some(host), "text/plain", "not from this page", secure, "").await;
    }
    let query = head.target.split_once('?').map_or("", |(_, q)| q);
    let token = query.split('&').find_map(|kv| kv.strip_prefix("token=")).unwrap_or("");
    if !same(token.as_bytes(), site.token.as_bytes()) {
        return reply(&mut stream, 403, Some(host), "text/plain", "reload the page", secure, "").await;
    }
    let has = |name: &str, word: &str| {
        head.get(name).iter().any(|v| v.split(',').any(|w| w.trim().eq_ignore_ascii_case(word)))
    };
    let key = head.one("Sec-WebSocket-Key").filter(|k| k.len() == 24);
    let Some(key) = key.filter(|_| has("Upgrade", "websocket") && has("Connection", "upgrade")) else {
        return reply(&mut stream, 400, Some(host), "text/plain", "not a WebSocket request", secure, "").await;
    };
    if head.one("Sec-WebSocket-Version") != Some("13") {
        return reply(&mut stream, 400, Some(host), "text/plain", "WebSocket version 13 only", secure, "").await;
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

/// Write one response and close. `host`: the checked Host, for the CSP's WebSocket source;
/// `secure`: over TLS (a `wss:` source); `extra`: more header lines, each ending in CRLF.
async fn reply(
    stream: &mut (impl AsyncWrite + Unpin),
    status: u16,
    host: Option<&str>,
    kind: &str,
    body: &str,
    secure: bool,
    extra: &str,
) {
    let reason = match status {
        200 => "OK",
        400 => "Bad Request",
        403 => "Forbidden",
        404 => "Not Found",
        308 => "Permanent Redirect",
        405 => "Method Not Allowed",
        421 => "Misdirected Request",
        _ => "Error",
    };
    // `'self'` covers the WebSocket too; an IPv6 literal isn't a CSP source, so it relies on that
    let scheme = if secure { "wss" } else { "ws" };
    let ws = host.filter(|h| !h.starts_with('[')).map(|h| format!(" {scheme}://{h}")).unwrap_or_default();
    let head = format!(
        "HTTP/1.1 {status} {reason}\r\n{extra}Content-Type: {kind}\r\nContent-Length: {}\r\nConnection: close\r\n\
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

/// The theme `block.watch` resolves screens with unless asked otherwise, as CSS variables, so the
/// page and the screens agree.
fn theme_css() -> String {
    let t = crate::screens::Palette::theme(None);
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
        let s = Site::new(&at, Some("box.example.ts.net"), 7297, "t".into(), None);
        assert_eq!(
            s.status(),
            (
                vec![
                    "http://box.example.ts.net:7297/".to_string(),
                    "http://100.64.0.5:7297/".into(),
                    "http://[fd7a:115c:a1e0::5]:7297/".into()
                ],
                None
            )
        );
        for ok in ["100.64.0.5:7297", "[FD7A:115C:A1E0::5]:7297", "Box.Example.ts.net.:7297", "box.example.ts.net:7297"]
        {
            assert!(s.own(ok).is_some(), "{ok}");
        }
        for bad in ["100.64.0.5", "box:7297", "evil.example:7297", "100.64.0.5:72970", "box.example.ts.net"] {
            assert!(s.own(bad).is_none(), "{bad}");
        }
        let s80 = Site::new(&["100.64.0.5:80".to_string()], None, 80, "t".into(), None);
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
