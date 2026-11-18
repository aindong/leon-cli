//! HTTPS for the web page's tests: a test CA with leaf certificates (rcgen), the files written as
//! `tailscale cert` writes them, and a TLS client that trusts only the test CA.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use rcgen::{date_time_ymd, BasicConstraints, CertificateParams, IsCa, Issuer, KeyPair};
use rustls::pki_types::{CertificateDer, ServerName};

use super::web::{parse, Reply};

pub struct Ca {
    issuer: Issuer<'static, KeyPair>,
    pub der: CertificateDer<'static>,
    pub pem: String,
}

/// A leaf's PEM files, as `tailscale cert --cert-file --key-file` writes them.
pub struct Leaf {
    pub cert: String,
    pub key: String,
}

/// When a leaf is valid: now, long expired, not yet, or until this many seconds from now.
#[derive(Clone, Copy)]
pub enum When {
    Now,
    Expired,
    Future,
    For(i64),
}

impl Ca {
    pub fn new() -> Ca {
        let mut p = CertificateParams::new(Vec::<String>::new()).unwrap();
        p.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        let key = KeyPair::generate().unwrap();
        let cert = p.self_signed(&key).unwrap();
        Ca { der: cert.der().clone(), pem: cert.pem(), issuer: Issuer::new(p, key) }
    }

    /// A leaf for `names` (an address or a DNS name each).
    pub fn leaf(&self, names: &[&str], when: When) -> Leaf {
        let mut p = CertificateParams::new(names.iter().map(|n| n.to_string()).collect::<Vec<_>>()).unwrap();
        match when {
            When::Now => {}
            When::Expired => (p.not_before, p.not_after) = (date_time_ymd(2000, 1, 1), date_time_ymd(2001, 1, 1)),
            When::Future => p.not_before = date_time_ymd(2100, 1, 1),
            When::For(secs) => p.not_after = time::OffsetDateTime::now_utc() + time::Duration::seconds(secs),
        }
        let key = KeyPair::generate().unwrap();
        let cert = p.signed_by(&key, &self.issuer).unwrap();
        Leaf { cert: cert.pem(), key: key.serialize_pem() }
    }
}

impl Leaf {
    /// Write `web.crt` and `web.key` (0600) in `dir`; returns their paths.
    pub fn write(&self, dir: &Path) -> (PathBuf, PathBuf) {
        let (cert, key) = (dir.join("web.crt"), dir.join("web.key"));
        std::fs::write(&cert, &self.cert).unwrap();
        std::fs::write(&key, &self.key).unwrap();
        std::fs::set_permissions(&key, std::fs::Permissions::from_mode(0o600)).unwrap();
        (cert, key)
    }
}

/// The Lua lines that turn the web page on with these files.
pub fn settings(cert: &Path, key: &Path) -> String {
    format!(
        "leon.set(\"serve_web\", true)\nleon.set(\"serve_web_cert\", {:?})\nleon.set(\"serve_web_key\", {:?})",
        cert.to_str().unwrap(),
        key.to_str().unwrap()
    )
}

/// A TLS connection to 127.0.0.1:`port` that trusts only `ca`; Err when the handshake fails.
pub fn connect(port: u16, ca: &Ca) -> Result<rustls::StreamOwned<rustls::ClientConnection, TcpStream>, String> {
    connect_as(port, ca, "127.0.0.1")
}

/// `connect`, asking for (and checking the certificate against) `name`, an address or a DNS name.
pub fn connect_as(
    port: u16,
    ca: &Ca,
    name: &str,
) -> Result<rustls::StreamOwned<rustls::ClientConnection, TcpStream>, String> {
    let mut roots = rustls::RootCertStore::empty();
    roots.add(ca.der.clone()).unwrap();
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let config = rustls::ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_root_certificates(roots)
        .with_no_client_auth();
    let name = ServerName::try_from(name.to_string()).unwrap();
    let conn = rustls::ClientConnection::new(Arc::new(config), name).unwrap();
    let tcp = TcpStream::connect(("127.0.0.1", port)).map_err(|e| e.to_string())?;
    tcp.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    let mut s = rustls::StreamOwned::new(conn, tcp);
    while s.conn.is_handshaking() {
        s.conn.complete_io(&mut s.sock).map_err(|e| e.to_string())?;
    }
    Ok(s)
}

/// The server's leaf certificate, as DER.
pub fn served_cert(port: u16, ca: &Ca) -> Result<Vec<u8>, String> {
    let s = connect(port, ca)?;
    Ok(s.conn.peer_certificates().ok_or("no certificate")?[0].to_vec())
}

/// `GET path` over TLS with `Host: host`; Err when the handshake fails.
pub fn get(port: u16, ca: &Ca, path: &str, host: &str) -> Result<Reply, String> {
    let name = host.rsplit_once(':').map_or(host, |(n, _)| n);
    let mut s = connect_as(port, ca, name)?;
    s.write_all(format!("GET {path} HTTP/1.1\r\nHost: {host}\r\n\r\n").as_bytes()).map_err(|e| e.to_string())?;
    let mut got = Vec::new();
    let _ = s.read_to_end(&mut got); // a close without close_notify is still the end
    parse(&got).ok_or_else(|| format!("not an HTTP reply: {:?}", String::from_utf8_lossy(&got)))
}

/// The token in the HTTPS page.
pub fn token(port: u16, ca: &Ca) -> String {
    let page = get(port, ca, "/", &super::web::host(port)).expect("the page over TLS");
    assert_eq!(page.status, 200, "{page:?}");
    let at = page.body.find("name=\"leon-token\" content=\"").expect("the page has the token") + 27;
    page.body[at..].split('"').next().unwrap().to_string()
}

/// A fake `tailscale` that records its argv, named by `LEON_TAILSCALE` for every server this test
/// binary starts (they inherit it): leon must never run `tailscale cert`.
pub fn fake_tailscale() -> &'static Path {
    static DIR: std::sync::OnceLock<tempfile::TempDir> = std::sync::OnceLock::new();
    let dir = DIR.get_or_init(|| {
        let d = tempfile::tempdir().unwrap();
        let bin = d.path().join("tailscale");
        std::fs::write(&bin, "#!/bin/sh\nprintf '%s\\n' \"$*\" >> \"$(dirname \"$0\")/argv\"\nexit 1\n").unwrap();
        std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).unwrap();
        // every test in the binary sets the same value: the race is harmless
        std::env::set_var("LEON_TAILSCALE", &bin);
        d
    });
    dir.path()
}

/// What the fake `tailscale` was run with, a line per run.
pub fn tailscale_runs() -> String {
    std::fs::read_to_string(fake_tailscale().join("argv")).unwrap_or_default()
}
