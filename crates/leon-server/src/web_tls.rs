//! HTTPS for the web page (docs/architecture/web.md): the certificate and key `tailscale cert`
//! wrote, named by `serve_web_cert` and `serve_web_key`. leon only reads them: it never runs
//! `tailscale cert` and never asks tailscaled for a certificate. The files are looked at on the
//! tailnet supervisor's tick: a change reloads them, a bad one keeps the last good certificate
//! while it is valid, and an expired one stops HTTPS. Every problem names the command to run.

use std::io::Read;
use std::net::IpAddr;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use rustls::pki_types::pem::PemObject;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, ServerName};
use rustls::ServerConfig;

/// The most of each file read: a chain of a few certificates is a few KB.
const MOST: u64 = 1 << 20;

/// HTTPS from the files the settings name.
pub(crate) struct Tls {
    /// The two paths (`~/` expanded), or why they can't be used.
    files: Result<(PathBuf, PathBuf), String>,
    /// The MagicDNS name the certificate should cover, and the addresses served.
    dns: Option<String>,
    ips: Vec<IpAddr>,
    state: Mutex<State>,
}

#[derive(Default)]
struct State {
    /// The files as last read (`stat`), so an unchanged pair isn't read again.
    seen: Option<[Stat; 2]>,
    good: Option<Good>,
    /// A certificate loaded before its notBefore: it takes over when that comes.
    waiting: Option<Good>,
    error: Option<String>,
}

#[derive(Clone)]
struct Good {
    config: Arc<ServerConfig>,
    /// When the leaf is valid, in seconds since the epoch.
    valid: (i64, i64),
    /// The hosts it covers, as `NAME` or `ADDRESS` (no port).
    names: Vec<String>,
}

/// What the page is served as now: the hosts to offer, or why there are none.
pub(crate) struct Now {
    pub(crate) names: Vec<String>,
    pub(crate) error: Option<String>,
}

impl Tls {
    /// None when neither setting is set (plain HTTP).
    pub(crate) fn new(cert: &str, key: &str, dns: Option<&str>, ips: &[IpAddr]) -> Option<Tls> {
        let files = match (cert.is_empty(), key.is_empty()) {
            (true, true) => return None,
            (false, false) => expand(cert).and_then(|c| Ok((c, expand(key)?))),
            (true, false) => Err("serve_web_key is set but serve_web_cert isn't: set both, or neither".into()),
            (false, true) => Err("serve_web_cert is set but serve_web_key isn't: set both, or neither".into()),
        };
        Some(Tls { files, dns: dns.map(str::to_string), ips: ips.to_vec(), state: Mutex::default() })
    }

    /// The hosts the loaded certificate covers (none without one).
    pub(crate) fn names(&self) -> Vec<String> {
        crate::events::lock(&self.state).good.as_ref().map(|g| g.names.clone()).unwrap_or_default()
    }

    /// The config new connections use, if a valid certificate is loaded.
    pub(crate) fn config(&self) -> Option<Arc<ServerConfig>> {
        crate::events::lock(&self.state).good.as_ref().map(|g| g.config.clone())
    }

    /// Look at the files: reload them if they changed, take up a waiting certificate whose time has
    /// come, drop an expired one. Returns what is served now; the error is new or the same as before.
    /// Only the supervisor calls it, once a tick.
    pub(crate) fn refresh(&self) -> Now {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64);
        let (cert, key) = match &self.files {
            Ok(f) => f,
            Err(e) => {
                crate::events::lock(&self.state).error = Some(e.clone());
                return Now { names: vec![], error: Some(e.clone()) };
            }
        };
        // looked at before reading, so a change that lands while reading is seen next time
        let seen = [stat(cert), stat(key)];
        let changed = crate::events::lock(&self.state).seen != Some(seen);
        // read without the lock: TLS accepts go on with the current certificate meanwhile
        let loaded = changed.then(|| self.load(cert, key));
        let advice = || self.advice(cert, key);
        let expired = || format!("the certificate in {} is no longer valid (expired). {}", cert.display(), advice());
        let mut st = crate::events::lock(&self.state);
        st.seen = Some(seen);
        match loaded {
            None => {}
            Some(Err(e)) => (st.waiting, st.error) = (None, Some(format!("{e}. {}", advice()))),
            Some(Ok(g)) if now < g.valid.0 => {
                let why = format!("the certificate in {} isn't valid yet. {}", cert.display(), advice());
                (st.waiting, st.error) = (Some(g), Some(why));
            }
            Some(Ok(g)) if now > g.valid.1 => (st.waiting, st.error) = (None, Some(expired())),
            Some(Ok(g)) => (st.good, st.waiting, st.error) = (Some(g), None, None),
        }
        if st.waiting.as_ref().is_some_and(|w| now >= w.valid.0) {
            (st.good, st.error) = (st.waiting.take(), None);
        }
        if st.good.as_ref().is_some_and(|g| now > g.valid.1) {
            (st.good, st.error) = (None, Some(expired())); // notAfter is the last valid second
        }
        Now { names: st.good.as_ref().map(|g| g.names.clone()).unwrap_or_default(), error: st.error.clone() }
    }

    fn load(&self, cert: &Path, key: &Path) -> Result<Good, String> {
        let (chain, _) = read(cert)?;
        let chain: Vec<CertificateDer<'static>> = CertificateDer::pem_slice_iter(&chain)
            .collect::<Result<_, _>>()
            .map_err(|_| format!("{} isn't a PEM certificate", cert.display()))?;
        let leaf = chain.first().ok_or_else(|| format!("{} has no certificate in it", cert.display()))?;
        let (key_pem, meta) = read(key)?;
        // the open file's own owner and mode: a symlink swapped meanwhile can't get past this
        // SAFETY: geteuid has no preconditions.
        if meta.uid() != unsafe { libc::geteuid() } {
            return Err(format!("{} isn't yours: sudo chown \"$USER\" {}", key.display(), quoted(key)));
        }
        if meta.mode() & 0o077 != 0 {
            return Err(format!("{} can be read by others: chmod 600 {}", key.display(), quoted(key)));
        }
        let private = PrivateKeyDer::from_pem_slice(&key_pem)
            .map_err(|_| format!("{} isn't a PEM private key", key.display()))?;
        let valid = validity(leaf).ok_or_else(|| format!("can't read the dates in {}", cert.display()))?;
        let names = self.covered(leaf)?;
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let mut config = ServerConfig::builder_with_provider(provider)
            .with_safe_default_protocol_versions()
            .map_err(|e| e.to_string())?
            .with_no_client_auth()
            .with_single_cert(chain, private)
            .map_err(|e| {
                format!("the key in {} doesn't match the certificate in {}: {e}", key.display(), cert.display())
            })?;
        config.alpn_protocols = vec![b"http/1.1".to_vec()];
        Ok(Good { config: Arc::new(config), valid, names })
    }

    /// The hosts `leaf` covers: the MagicDNS name (it must, when there is one), and any address.
    fn covered(&self, leaf: &CertificateDer<'_>) -> Result<Vec<String>, String> {
        let ee = webpki::EndEntityCert::try_from(leaf).map_err(|e| format!("can't read the certificate: {e:?}"))?;
        let covers = |n: &str| {
            ServerName::try_from(n.to_string()).is_ok_and(|s| ee.verify_is_valid_for_subject_name(&s).is_ok())
        };
        let mut names: Vec<String> = self.dns.iter().filter(|d| covers(d)).cloned().collect();
        if let Some(d) = self.dns.as_ref().filter(|_| names.is_empty()) {
            return Err(format!("the certificate isn't for this machine's name {d}"));
        }
        names.extend(self.ips.iter().map(|ip| ip.to_string()).filter(|ip| covers(ip)));
        match names.is_empty() {
            true => Err("the certificate isn't for this machine's name or addresses".into()),
            false => Ok(names),
        }
    }

    /// What to run: `tailscale cert` writes both files.
    fn advice(&self, cert: &Path, key: &Path) -> String {
        let name = self.dns.as_deref().unwrap_or("MACHINE.TAILNET.ts.net");
        format!(
            "Make them with: {}",
            leon_hosts::tailscale::cert_command(&leon_hosts::tailscale::cert_tool(), cert, key, name)
        )
    }
}

/// `~/x` under `$HOME`; anything else as written (the setting takes only `/` and `~/`).
fn expand(path: &str) -> Result<PathBuf, String> {
    match (path.strip_prefix("~/"), std::env::var_os("HOME")) {
        (Some(rest), Some(home)) => Ok(Path::new(&home).join(rest)),
        (Some(_), None) => Err(format!("{path} starts with ~/ but the server has no HOME: give the whole path")),
        (None, _) => Ok(PathBuf::from(path)),
    }
}

/// A path as one shell word, for a command to paste.
fn quoted(p: &Path) -> String {
    format!("'{}'", p.display().to_string().replace('\'', "'\\''"))
}

/// Size, inode, and modification and change times to the nanosecond: a file rewritten in place
/// with the same size in the same second still differs.
type Stat = (u64, u64, i64, i64, i64, i64);

fn stat(p: &Path) -> Stat {
    std::fs::metadata(p)
        .map_or((0, 0, 0, 0, 0, 0), |m| (m.len(), m.ino(), m.mtime(), m.mtime_nsec(), m.ctime(), m.ctime_nsec()))
}

/// The whole file (at most `MOST`), and the open file's metadata.
fn read(p: &Path) -> Result<(Vec<u8>, std::fs::Metadata), String> {
    use std::os::unix::fs::OpenOptionsExt;
    // non-blocking, so a FIFO (or a device) at the path can't hold the open; then refused below
    let f =
        std::fs::OpenOptions::new().read(true).custom_flags(libc::O_NONBLOCK).open(p).map_err(|e| match e.kind() {
            std::io::ErrorKind::PermissionDenied => {
                format!("leon can't read {}: sudo chown \"$USER\" {1} && chmod 600 {1}", p.display(), quoted(p))
            }
            _ => format!("can't read {}: {e}", p.display()),
        })?;
    let meta = f.metadata().map_err(|e| format!("can't read {}: {e}", p.display()))?;
    if !meta.is_file() {
        return Err(format!("{} isn't a regular file", p.display()));
    }
    let mut buf = Vec::new();
    f.take(MOST + 1).read_to_end(&mut buf).map_err(|e| format!("can't read {}: {e}", p.display()))?;
    if buf.len() as u64 > MOST {
        return Err(format!("{} is bigger than a certificate or key file", p.display()));
    }
    Ok((buf, meta))
}

/// The leaf's notBefore and notAfter, in seconds since the epoch: a walk of the DER to
/// TBSCertificate's validity. Every step checks its length against what is left.
fn validity(der: &[u8]) -> Option<(i64, i64)> {
    let (cert, _) = tlv(der, 0x30)?;
    let (mut tbs, _) = tlv(cert, 0x30)?;
    if tbs.first() == Some(&0xA0) {
        tbs = tlv(tbs, 0xA0)?.1; // version
    }
    tbs = tlv(tbs, 0x02)?.1; // serial
    tbs = tlv(tbs, 0x30)?.1; // signature algorithm
    tbs = tlv(tbs, 0x30)?.1; // issuer
    let (validity, _) = tlv(tbs, 0x30)?;
    let (from, rest) = time(validity)?;
    let (until, _) = time(rest)?;
    Some((from, until))
}

/// One DER element with tag `tag`: its contents and what follows it.
fn tlv(b: &[u8], tag: u8) -> Option<(&[u8], &[u8])> {
    let (&t, b) = b.split_first()?;
    let (&first, mut b) = b.split_first()?;
    if t != tag {
        return None;
    }
    let len = match first {
        n if n < 0x80 => n as usize,
        n => {
            let count = (n & 0x7f) as usize;
            if count == 0 || count > 4 || b.len() < count {
                return None;
            }
            let len = b[..count].iter().fold(0usize, |l, &x| l << 8 | x as usize);
            b = &b[count..];
            len
        }
    };
    (b.len() >= len).then(|| b.split_at(len))
}

/// A UTCTime or GeneralizedTime (UTC, seconds), and what follows it.
fn time(b: &[u8]) -> Option<(i64, &[u8])> {
    let (digits, rest, year) = match b.first()? {
        0x17 => {
            let (d, rest) = tlv(b, 0x17)?;
            let yy = d.get(..2).filter(|y| y.iter().all(u8::is_ascii_digit))?;
            let yy = (yy[0] - b'0') as i64 * 10 + (yy[1] - b'0') as i64;
            (d.get(2..)?, rest, if yy < 50 { 2000 + yy } else { 1900 + yy })
        }
        0x18 => {
            let (d, rest) = tlv(b, 0x18)?;
            let y = d.get(..4).filter(|y| y.iter().all(u8::is_ascii_digit))?;
            (d.get(4..)?, rest, y.iter().fold(0i64, |n, b| n * 10 + (b - b'0') as i64))
        }
        _ => return None,
    };
    if digits.len() != 11 || digits[10] != b'Z' || !digits[..10].iter().all(u8::is_ascii_digit) {
        return None;
    }
    let n = |i: usize| (digits[i] - b'0') as i64 * 10 + (digits[i + 1] - b'0') as i64;
    let (mo, d, h, mi, s) = (n(0), n(2), n(4), n(6), n(8));
    if !(1..=12).contains(&mo) || !(1..=31).contains(&d) || h > 23 || mi > 59 || s > 60 {
        return None;
    }
    Some((days(year, mo, d) * 86400 + h * 3600 + mi * 60 + s, rest))
}

/// Days from 1970-01-01 to a date (Howard Hinnant's days_from_civil).
fn days(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_read_from_der() {
        assert_eq!(days(1970, 1, 1), 0);
        assert_eq!(days(2000, 3, 1), 11017);
        let utc = [0x17, 13, b'2', b'6', b'1', b'1', b'1', b'9', b'0', b'0', b'0', b'0', b'0', b'0', b'Z'];
        assert_eq!(time(&utc).map(|t| t.0), Some(1_795_046_400));
        let mut gen = vec![0x18, 15];
        gen.extend_from_slice(b"20991231235959Z");
        assert_eq!(time(&gen).map(|t| t.0), Some(4_102_444_799));
        let utc50 = [0x17, 13, b'5', b'0', b'0', b'1', b'0', b'1', b'0', b'0', b'0', b'0', b'0', b'0', b'Z'];
        assert_eq!(time(&utc50).map(|t| t.0), Some(-631_152_000), "YY 50 is 1950");
        assert_eq!(time(&[0x17, 13, b'2']), None, "a length past the end");
        assert_eq!(tlv(&[0x30, 0x84, 0xff, 0xff, 0xff, 0xff], 0x30), None, "a huge length");
        assert_eq!(tlv(&[0x30, 0x85, 0, 0, 0, 0, 1], 0x30), None, "a length of more than 4 bytes");
        assert_eq!(validity(&[0x30, 0x02, 0x30, 0x00]), None, "a certificate with nothing in it");
        let signed = [0x17, 13, b'+', b'6', b'1', b'1', b'1', b'9', b'0', b'0', b'0', b'0', b'0', b'0', b'Z'];
        assert_eq!(time(&signed), None, "digits only");
        let month13 = [0x17, 13, b'2', b'6', b'1', b'3', b'1', b'9', b'0', b'0', b'0', b'0', b'0', b'0', b'Z'];
        assert_eq!(time(&month13), None, "a month past 12");
    }

    #[test]
    fn settings_say_what_is_missing() {
        assert!(Tls::new("", "", None, &[]).is_none(), "neither: plain HTTP");
        let one = Tls::new("/a.crt", "", None, &[]).unwrap();
        assert!(one.refresh().error.unwrap().contains("serve_web_key"));
        assert_eq!(expand("/x"), Ok(PathBuf::from("/x")));
        if let Some(home) = std::env::var_os("HOME") {
            assert_eq!(expand("~/c/x.crt"), Ok(Path::new(&home).join("c/x.crt")));
        }
        assert_eq!(quoted(Path::new("/a b/it's")), "'/a b/it'\\''s'");
    }
}
