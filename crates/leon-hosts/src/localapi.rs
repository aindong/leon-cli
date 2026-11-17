//! tailscaled's LocalAPI, read only: `status` and `whois`, the two calls leon makes
//! (docs/architecture/tailnet.md). A server serving over Tailscale asks who each caller is; a
//! client asks who a `tailnet://` destination is. Both decide with `admit`. Replies are another
//! program's bytes: bounded in size and time, every field checked, a missing one refuses.

use std::io::{Read, Write};
use std::net::{IpAddr, SocketAddr, TcpStream};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use serde_json::Value;

use crate::shown_text;

/// Each call gets this long, connect to last byte.
pub const DEADLINE: Duration = Duration::from_secs(2);
/// `status?peers=false` is this machine alone: a few KB.
const MOST_STATUS: usize = 4 << 20;
const MOST_WHOIS: usize = 1 << 20;
/// Where tailscaled listens when it is a daemon with a Unix socket (Linux, the open-source macOS build).
const SOCKETS: &[&str] = if cfg!(target_os = "macos") {
    &["/var/run/tailscaled.socket"]
} else {
    &["/var/run/tailscale/tailscaled.sock", "/run/tailscale/tailscaled.sock"]
};
/// The Mac app's LocalAPI: `ipnport` links to its localhost port, `sameuserproof-PORT` holds the token.
const MAC_APP: &str = "/Library/Tailscale";

/// The tests' way in, compiled only into debug builds: `LEON_TEST_TAILNET` =
/// `{"localapi": SOCKET, "loopback": true}` names a fake LocalAPI and lets loopback stand in for
/// the tailnet. A release build has no code that reads it (update_release_build checks).
#[cfg(debug_assertions)]
fn seam() -> Option<Value> {
    let text = std::env::var("LEON_TEST_TAILNET").ok()?;
    Some(serde_json::from_str(&text).expect("LEON_TEST_TAILNET is JSON"))
}

#[cfg(not(debug_assertions))]
fn seam() -> Option<Value> {
    None
}

/// Loopback addresses count as the tailnet (tests only).
pub fn loopback_is_tailnet() -> bool {
    seam().is_some_and(|s| s["loopback"] == true)
}

/// This machine on the tailnet, from `status?peers=false`.
#[derive(Debug, Clone, PartialEq)]
pub struct Me {
    /// Its node's StableID (`Self.ID`).
    pub node: String,
    /// The user it is signed in as: the id and the login exactly as tailscaled wrote them.
    pub user: u64,
    pub login: String,
    /// Its Tailscale addresses (at most 8), unchecked: `direct::on_tailnet` picks.
    pub ips: Vec<IpAddr>,
    pub tagged: bool,
}

/// A caller or a destination, from `whois`.
#[derive(Debug, Clone, PartialEq)]
pub struct Peer {
    pub node: String,
    pub user: Option<u64>,
    pub login: Option<String>,
    pub tags: Vec<String>,
}

/// Whether `peer` may reach `me`'s server (or be reached by `me`): one of `me`'s own untagged
/// machines, not `me` itself, or a tagged one with a tag in `tags`. The error is the reason.
pub fn admit(me: &Me, peer: &Peer, tags: &[String]) -> Result<(), String> {
    if peer.node == me.node {
        return Err("it is this machine itself (another user or program here)".into());
    }
    if !peer.tags.is_empty() {
        let shown = shown_text(&peer.tags.join(", "), 200);
        return match peer.tags.iter().any(|t| tags.contains(t)) {
            true => Ok(()),
            false => Err(format!("it is a tagged machine ({shown}), not one of yours")),
        };
    }
    match (peer.user, &peer.login) {
        (Some(u), Some(l)) if u == me.user && *l == me.login && l != "tagged-devices" => Ok(()),
        (_, Some(l)) if *l == me.login => Err(format!("it is another account with the login {}", shown_text(l, 128))),
        (_, Some(l)) => Err(format!(
            "it is signed in to Tailscale as {}, not as {}",
            shown_text(l, 128),
            shown_text(&me.login, 128)
        )),
        _ => Err("tailscaled didn't say who it is".into()),
    }
}

/// This machine's state on the tailnet; an error when Tailscale isn't running or signed in.
pub fn status() -> Result<Me, String> {
    parse_status(&get("/localapi/v0/status?peers=false", MOST_STATUS)?)
}

/// Who is at `addr` on the tailnet.
pub fn whois(addr: SocketAddr) -> Result<Peer, String> {
    let query: String = addr
        .to_string()
        .bytes()
        .map(|b| match b.is_ascii_alphanumeric() || b == b'.' {
            true => (b as char).to_string(),
            false => format!("%{b:02X}"),
        })
        .collect();
    parse_whois(&get(&format!("/localapi/v0/whois?addr={query}"), MOST_WHOIS)?)
}

pub fn parse_status(body: &[u8]) -> Result<Me, String> {
    let bad = |why: &str| format!("can't read Tailscale's status: {why}");
    let v: Value = serde_json::from_slice(body).map_err(|_| bad("not JSON"))?;
    match v.get("BackendState").and_then(Value::as_str) {
        Some("Running") => {}
        Some("Stopped") => return Err("Tailscale is stopped".into()),
        Some("NeedsLogin") => return Err("Tailscale needs a login".into()),
        Some(s) => return Err(format!("Tailscale isn't running (it is {})", shown_text(s, 32).to_lowercase())),
        None => return Err(bad("no BackendState")),
    }
    let me = v.get("Self").filter(|s| s.is_object()).ok_or_else(|| bad("no Self"))?;
    let node = me.get("ID").and_then(Value::as_str).filter(|n| !n.is_empty()).ok_or_else(|| bad("no Self.ID"))?;
    let user = me.get("UserID").and_then(Value::as_u64).ok_or_else(|| bad("no Self.UserID"))?;
    let login = v.pointer(&format!("/User/{user}/LoginName")).and_then(Value::as_str).filter(|l| !l.is_empty());
    let login = login.ok_or_else(|| bad("no login for this machine's user"))?;
    let ips = match me.get("TailscaleIPs") {
        Some(Value::Array(a)) => a.iter().filter_map(Value::as_str).filter_map(|ip| ip.parse().ok()).take(8).collect(),
        _ => Vec::new(),
    };
    let tagged = !tags(me.get("Tags")).ok_or_else(|| bad("Self.Tags isn't a list"))?.is_empty();
    Ok(Me { node: node.to_string(), user, login: login.to_string(), ips, tagged })
}

pub fn parse_whois(body: &[u8]) -> Result<Peer, String> {
    let bad = |why: &str| format!("can't read Tailscale's whois: {why}");
    let v: Value = serde_json::from_slice(body).map_err(|_| bad("not JSON"))?;
    let node = v.get("Node").filter(|n| n.is_object()).ok_or_else(|| bad("no Node"))?;
    let id =
        node.get("StableID").and_then(Value::as_str).filter(|n| !n.is_empty()).ok_or_else(|| bad("no StableID"))?;
    let tags = tags(node.get("Tags")).ok_or_else(|| bad("Tags isn't a list"))?;
    let profile = v.get("UserProfile");
    Ok(Peer {
        node: id.to_string(),
        user: profile.and_then(|p| p.get("ID")).and_then(Value::as_u64),
        login: profile.and_then(|p| p.get("LoginName")).and_then(Value::as_str).map(str::to_string),
        tags,
    })
}

/// A node's tags: none when the field is absent or null (Tailscale leaves it out for an untagged
/// node); None, a refusal, for anything but a list of strings.
fn tags(v: Option<&Value>) -> Option<Vec<String>> {
    match v {
        None | Some(Value::Null) => Some(Vec::new()),
        Some(Value::Array(a)) => a.iter().map(|t| t.as_str().map(str::to_string)).take(64).collect(),
        Some(_) => None,
    }
}

enum Api {
    /// A socket, and the uid its listener must have (root; ours for the tests' fake).
    Socket(PathBuf, u32),
    /// The Mac app's localhost port and token.
    Tcp(u16, String),
}

fn not_found() -> String {
    "can't reach Tailscale's LocalAPI: is Tailscale running? (the Mac App Store build isn't supported: use the \
     standalone app)"
        .into()
}

fn locate() -> Result<Api, String> {
    if let Some(s) = seam() {
        let path = s["localapi"].as_str().ok_or("LEON_TEST_TAILNET names no localapi")?;
        // SAFETY: geteuid has no preconditions.
        return Ok(Api::Socket(path.into(), unsafe { libc::geteuid() }));
    }
    if cfg!(target_os = "macos") {
        if let Some(api) = mac_app(Path::new(MAC_APP))? {
            return Ok(api);
        }
    }
    let found = SOCKETS.iter().map(PathBuf::from).find(|p| std::fs::symlink_metadata(p).is_ok());
    found.map(|p| Api::Socket(p, 0)).ok_or_else(not_found)
}

/// Root's and nobody else's to change: else another user could point leon at their own LocalAPI.
fn root_only(path: &Path, m: &std::fs::Metadata) -> Result<(), String> {
    match m.uid() == 0 && m.mode() & 0o022 == 0 {
        true => Ok(()),
        false => Err(format!("refusing Tailscale's LocalAPI: {} isn't root's alone", path.display())),
    }
}

/// The Mac app's port and token, when it is installed.
fn mac_app(dir: &Path) -> Result<Option<Api>, String> {
    let link = dir.join("ipnport");
    let Ok(m) = std::fs::symlink_metadata(&link) else { return Ok(None) };
    root_only(dir, &std::fs::symlink_metadata(dir).map_err(|e| e.to_string())?)?;
    root_only(&link, &m)?;
    let target = std::fs::read_link(&link).map_err(|e| format!("can't read {}: {e}", link.display()))?;
    let port = target.to_str().filter(|p| (1..=5).contains(&p.len()) && p.bytes().all(|b| b.is_ascii_digit()));
    let port: u16 = port
        .and_then(|p| p.parse().ok())
        .filter(|&p| p > 0)
        .ok_or_else(|| format!("{} isn't a port", link.display()))?;
    let proof = dir.join(format!("sameuserproof-{port}"));
    let mut f = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(&proof)
        .map_err(|e| format!("can't read Tailscale's LocalAPI token: {e}"))?;
    root_only(&proof, &f.metadata().map_err(|e| e.to_string())?)?;
    let mut token = String::new();
    (&mut f).take(256).read_to_string(&mut token).map_err(|e| format!("can't read Tailscale's LocalAPI token: {e}"))?;
    let token = token.trim().to_string();
    match !token.is_empty() && token.bytes().all(|b| b.is_ascii_graphic()) {
        true => Ok(Some(Api::Tcp(port, token))),
        false => Err("Tailscale's LocalAPI token is unreadable".into()),
    }
}

/// GET `path` with HTTP/1.0 (a plain body that ends at the close): the body of a 200, at most `most` bytes.
fn get(path: &str, most: usize) -> Result<Vec<u8>, String> {
    let end = Instant::now() + DEADLINE;
    let down = |e: std::io::Error| format!("Tailscale's LocalAPI isn't answering: {e}");
    let mut auth = String::new();
    let mut stream: Box<dyn ReadWrite> = match locate()? {
        Api::Socket(path, uid) => {
            let s = UnixStream::connect(&path).map_err(down)?;
            if crate::peer_uid(&s) != Some(uid) {
                return Err(format!("refusing Tailscale's LocalAPI: {} isn't tailscaled's", path.display()));
            }
            s.set_write_timeout(Some(DEADLINE)).map_err(down)?;
            Box::new(s)
        }
        Api::Tcp(port, token) => {
            let s = TcpStream::connect_timeout(&SocketAddr::from(([127, 0, 0, 1], port)), DEADLINE).map_err(down)?;
            s.set_write_timeout(Some(DEADLINE)).map_err(down)?;
            auth = format!("Authorization: Basic {}\r\n", base64(format!(":{token}").as_bytes()));
            Box::new(s)
        }
    };
    let req = format!("GET {path} HTTP/1.0\r\nHost: local-tailscaled.sock\r\n{auth}\r\n");
    // once, before the write: macOS refuses to set it on a socket whose peer already closed
    stream.timeout(DEADLINE).map_err(down)?;
    stream.write_all(req.as_bytes()).map_err(down)?;
    let mut got = Vec::new();
    let mut buf = [0u8; 16 << 10];
    let late = || format!("Tailscale's LocalAPI didn't answer in {} s", DEADLINE.as_secs());
    loop {
        // each read waits at most DEADLINE, so a trickle ends by twice it
        if Instant::now() >= end {
            return Err(late());
        }
        match stream.read(&mut buf) {
            Ok(0) => break,
            Ok(n) if got.len() + n <= most + (16 << 10) => got.extend_from_slice(&buf[..n]),
            Ok(_) => return Err("Tailscale's LocalAPI answer is too large".into()),
            Err(e) if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => {
                return Err(late())
            }
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(e) => return Err(down(e)),
        }
    }
    let head_end =
        got.windows(4).position(|w| w == b"\r\n\r\n").ok_or("Tailscale's LocalAPI answer has no end of headers")?;
    let status = got.split(|&b| b == b'\n').next().unwrap_or_default();
    let status = String::from_utf8_lossy(status);
    let mut words = status.split(' ');
    let (proto, code) = (words.next().unwrap_or(""), words.next().unwrap_or(""));
    if !matches!(proto, "HTTP/1.0" | "HTTP/1.1") || code != "200" {
        return Err(format!("Tailscale's LocalAPI said {}", shown_text(&status, 60)));
    }
    Ok(got.split_off(head_end + 4))
}

trait ReadWrite: Read + Write {
    fn timeout(&self, d: Duration) -> std::io::Result<()>;
}

impl ReadWrite for UnixStream {
    fn timeout(&self, d: Duration) -> std::io::Result<()> {
        self.set_read_timeout(Some(d))
    }
}

impl ReadWrite for TcpStream {
    fn timeout(&self, d: Duration) -> std::io::Result<()> {
        self.set_read_timeout(Some(d))
    }
}

fn base64(bytes: &[u8]) -> String {
    const ABC: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for c in bytes.chunks(3) {
        let n = c.iter().enumerate().fold(0u32, |n, (i, &b)| n | (b as u32) << (16 - 8 * i));
        for i in 0..4 {
            out.push(if i <= c.len() { ABC[(n >> (18 - 6 * i) & 63) as usize] as char } else { '=' });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn me() -> Me {
        Me { node: "nself".into(), user: 7, login: "me@example.com".into(), ips: vec![], tagged: false }
    }

    fn peer(node: &str, user: Option<u64>, login: Option<&str>, tags: &[&str]) -> Peer {
        Peer {
            node: node.into(),
            user,
            login: login.map(str::to_string),
            tags: tags.iter().map(|t| t.to_string()).collect(),
        }
    }

    #[test]
    fn only_your_own_other_untagged_machines_are_admitted() {
        let (m, none) = (me(), &[][..]);
        assert_eq!(admit(&m, &peer("n2", Some(7), Some("me@example.com"), &[]), none), Ok(()));
        let refused = [
            peer("nself", Some(7), Some("me@example.com"), &[]),
            peer("n2", Some(8), Some("me@example.com"), &[]),
            peer("n2", Some(7), Some("you@example.com"), &[]),
            peer("n2", Some(7), Some("me@example.com "), &[]),
            peer("n2", None, Some("me@example.com"), &[]),
            peer("n2", Some(7), None, &[]),
            peer("n2", Some(7), Some("me@example.com"), &["tag:ci"]),
        ];
        for p in refused {
            assert!(admit(&m, &p, none).is_err(), "{p:?}");
        }
        let tagged = Me { login: "tagged-devices".into(), ..me() };
        assert!(admit(&tagged, &peer("n2", Some(7), Some("tagged-devices"), &[]), none).is_err());
        let ci = ["tag:ci".to_string()];
        assert_eq!(admit(&m, &peer("n2", None, Some("tagged-devices"), &["tag:x", "tag:ci"]), &ci), Ok(()));
        assert!(admit(&m, &peer("n2", None, None, &["tag:cix"]), &ci).is_err(), "tags match exactly");
        assert!(admit(&m, &peer("nself", None, None, &["tag:ci"]), &ci).is_err(), "never this machine");
    }

    #[test]
    fn replies_are_read_strictly() {
        let st = br#"{"BackendState":"Running","Self":{"ID":"nself","UserID":7,"TailscaleIPs":["100.64.0.1","x"]},
            "User":{"7":{"LoginName":"me@example.com"}}}"#;
        let m = parse_status(st).unwrap();
        assert_eq!((m.node.as_str(), m.user, m.login.as_str(), m.tagged), ("nself", 7, "me@example.com", false));
        assert_eq!(m.ips, ["100.64.0.1".parse::<IpAddr>().unwrap()]);
        for bad in [
            &b"{}"[..],
            b"[]",
            br#"{"BackendState":"NeedsLogin"}"#,
            br#"{"BackendState":"Running","Self":{"ID":"n","UserID":"7"}}"#,
            br#"{"BackendState":"Running","Self":{"ID":"n","UserID":7,"Tags":"tag:x"},"User":{"7":{"LoginName":"a"}}}"#,
        ] {
            assert!(parse_status(bad).is_err(), "{}", String::from_utf8_lossy(bad));
        }
        let w =
            parse_whois(br#"{"Node":{"StableID":"n2"},"UserProfile":{"ID":7,"LoginName":"me@example.com"}}"#).unwrap();
        assert_eq!(w, peer("n2", Some(7), Some("me@example.com"), &[]));
        for bad in [
            &br#"{"Node":{"StableID":"n2","Tags":{"a":1}}}"#[..],
            br#"{"Node":{"StableID":"n2","Tags":[1]}}"#,
            br#"{"Node":{}}"#,
            br#"{"UserProfile":{"ID":7}}"#,
        ] {
            assert!(parse_whois(bad).is_err(), "{}", String::from_utf8_lossy(bad));
        }
    }

    #[test]
    fn the_mac_apps_files_must_be_roots_alone() {
        // a dir of ours, as another user's would be: refused before any token is read
        let d = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink("49169", d.path().join("ipnport")).unwrap();
        std::fs::write(d.path().join("sameuserproof-49169"), "t").unwrap();
        let e = mac_app(d.path()).err().unwrap();
        assert!(e.contains("isn't root's alone"), "{e}");
        assert!(mac_app(&d.path().join("none")).unwrap().is_none(), "no app: look elsewhere");
    }

    #[test]
    fn base64_is_standard() {
        assert_eq!(base64(b":tok"), "OnRvaw==");
        assert_eq!(base64(b":abc123"), "OmFiYzEyMw==");
        assert_eq!(base64(b":ab"), "OmFi");
    }
}
