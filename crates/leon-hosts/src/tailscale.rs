//! The tailnet's machines, from the local Tailscale client (docs/architecture/hosts.md,
//! "Tailscale"). leon only ever runs `tailscale status --json`, and reads its output as another
//! program's: bounded in size and time, every field optional, every name checked again by the
//! endpoint and label parsers before it can become a host.

use std::ffi::OsStr;
use std::io::Read;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::Value;

use crate::{hosts, shown_text, Endpoint, Host};

/// Where the client is when it isn't on PATH: the Mac app's binary is the CLI when given arguments.
pub const BUNDLES: &[&str] =
    if cfg!(target_os = "macos") { &["/Applications/Tailscale.app/Contents/MacOS/Tailscale"] } else { &[] };
/// A tailnet of thousands of machines is a few MB of status.
const MOST_BYTES: u64 = 8 << 20;
const MOST_PEERS: usize = 4096;
const DEADLINE: Duration = Duration::from_secs(3);

/// The client to run: `env` (`$LEON_TAILSCALE`) when set (none if it names nothing runnable),
/// else an executable `tailscale` in an absolute PATH entry, else the first bundle that exists. A
/// bare name in `env` is looked up in absolute PATH entries too, never in the working directory.
pub fn locate(env: Option<&OsStr>, path: Option<&OsStr>, bundles: &[&Path]) -> Option<PathBuf> {
    let runnable = |p: &Path| std::fs::metadata(p).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0);
    let on_path = |name: &OsStr| {
        let dirs = path.map(|p| std::env::split_paths(p).collect::<Vec<_>>()).unwrap_or_default();
        dirs.into_iter().filter(|d| d.is_absolute()).map(|d| d.join(name)).find(|p| runnable(p))
    };
    if let Some(e) = env.filter(|e| !e.is_empty()).map(Path::new) {
        return match e.components().count() == 1 && !e.is_absolute() {
            true => on_path(e.as_os_str()),
            false => runnable(e).then(|| e.to_path_buf()),
        };
    }
    on_path(OsStr::new("tailscale")).or_else(|| bundles.iter().find(|b| runnable(b)).map(|b| b.to_path_buf()))
}

/// A machine on the tailnet, as the status describes it.
#[derive(Debug, Clone, PartialEq)]
pub struct Machine {
    /// Its MagicDNS name without the trailing dot, as the status gave it (check it before use).
    pub dns: String,
    pub os: String,
    pub online: bool,
    /// It runs Tailscale SSH: the status lists its host keys only then.
    pub ssh: bool,
    /// Its first Tailscale address (IPv4 first), used when MagicDNS is off.
    pub ip: Option<String>,
    /// Its addresses, only ones inside Tailscale's ranges.
    pub ips: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Tailnet {
    /// The login this machine is on the tailnet with.
    pub login: Option<String>,
    pub magic_dns: bool,
    /// Online ones first, then by name.
    pub machines: Vec<Machine>,
}

impl Machine {
    /// Its short name: the first label of its MagicDNS name.
    pub fn name(&self) -> &str {
        self.dns.split('.').next().unwrap_or_default()
    }

    /// The name as a terminal or the app may show it.
    pub fn shown(&self) -> String {
        shown_text(&self.dns, 253)
    }

    /// `linux · online · Tailscale SSH`.
    pub fn marks(&self) -> String {
        let mut m = vec![shown_text(&self.os, 24)];
        m.push(if self.online { "online" } else { "offline" }.into());
        if self.ssh {
            m.push("Tailscale SSH".into());
        }
        m.retain(|s| !s.is_empty());
        m.join(" · ")
    }
}

impl Tailnet {
    /// The host entry for `m`: its short name as the label, its MagicDNS name (or, with MagicDNS
    /// off, its address) as the endpoint. Both go through the checks a typed host does.
    pub fn entry(&self, m: &Machine, icon: Option<String>) -> Result<Host, String> {
        hosts::check_label(m.name())?;
        let at = match self.magic_dns {
            true => m.dns.clone(),
            false => m.ip.clone().ok_or_else(|| format!("{} has no tailnet address", m.shown()))?,
        };
        let endpoint: Endpoint = match at.contains(':') {
            true => format!("ssh://[{at}]"),
            false => format!("ssh://{at}"),
        }
        .parse()?;
        // the name is only a host: an `@` or `:` in it must not pick the ssh user or port
        if endpoint.user.is_some() || endpoint.port.is_some() || endpoint.host != at {
            return Err(format!("bad machine name {:?}", m.shown()));
        }
        Ok(Host { label: m.name().to_string(), endpoints: vec![endpoint.to_string()], icon })
    }

    /// The machine `name` means: its MagicDNS name, else a short name only one machine has.
    pub fn find(&self, name: &str) -> Result<&Machine, String> {
        let name = name.trim_end_matches('.');
        if let Some(m) = self.machines.iter().find(|m| m.dns.eq_ignore_ascii_case(name)) {
            return Ok(m);
        }
        let mut short = self.machines.iter().filter(|m| m.name().eq_ignore_ascii_case(name));
        match (short.next(), short.next()) {
            (Some(m), None) => Ok(m),
            (Some(_), Some(_)) => Err(format!("{name:?} names more than one machine: use its MagicDNS name")),
            (None, _) => Err(format!("no machine {:?} on your tailnet", shown_text(name, 253))),
        }
    }

    /// The machine a host reaches: its first endpoint's host is a machine's MagicDNS name or address.
    /// shortcut: a host added by short name or through an ~/.ssh/config alias isn't matched.
    pub fn machine_for(&self, host: &Host) -> Option<&Machine> {
        let e = host.endpoint().ok()?;
        let at = e.host.trim_end_matches('.');
        self.machines.iter().find(|m| m.dns.eq_ignore_ascii_case(at) || m.ips.iter().any(|ip| ip == at))
    }

    /// What `leon hosts` and the sidebar say about a host on the tailnet.
    pub fn identity(&self) -> String {
        match &self.login {
            Some(l) => format!("tailnet: {l}"),
            None => "tailnet".into(),
        }
    }
}

/// What to do about a machine that doesn't run Tailscale SSH.
pub fn ssh_advice(label: &str) -> String {
    format!(
        "{label} doesn't offer Tailscale SSH: turn it on there with `tailscale set --ssh`, or set up an ssh key \
         for it"
    )
}

/// The client leon would run: `locate` with this process's env.
pub fn client() -> Option<PathBuf> {
    let bundles: Vec<&Path> = BUNDLES.iter().map(Path::new).collect();
    locate(std::env::var_os("LEON_TAILSCALE").as_deref(), std::env::var_os("PATH").as_deref(), &bundles)
}

/// Read the tailnet from the client leon finds.
pub fn status() -> Result<Tailnet, String> {
    let program = client().ok_or_else(not_installed)?;
    parse(&run(&program)?)
}

fn not_installed() -> String {
    match BUNDLES.first() {
        Some(b) => format!("Tailscale isn't installed: no tailscale on PATH or at {b}"),
        None => "Tailscale isn't installed: no tailscale on PATH".into(),
    }
}

/// `program status --json`'s stdout, at most `MOST_BYTES`, within `DEADLINE`. The read runs on a
/// thread left behind on timeout: a child of the client may hold the pipe after the client is killed.
fn run(program: &Path) -> Result<Vec<u8>, String> {
    let mut cmd = Command::new(program);
    cmd.args(["status", "--json"]).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null());
    leon_core::spawn::stdio_only(&mut cmd);
    let mut child = match cmd.spawn() {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Err(not_installed()),
        Err(e) => return Err(format!("can't run {}: {e}", program.display())),
        Ok(c) => c,
    };
    let out = child.stdout.take().expect("piped");
    let (tx, rx) = std::sync::mpsc::channel();
    let reader = move || {
        let mut buf = Vec::new();
        let read = out.take(MOST_BYTES + 1).read_to_end(&mut buf);
        let _ = tx.send(read.map(|_| buf));
    };
    if std::thread::Builder::new().name("leon-tailscale".into()).spawn(reader).is_err() {
        let _ = child.kill();
        let _ = child.wait();
        return Err("can't read tailscale's status: no thread".into());
    }
    let end = Instant::now() + DEADLINE;
    let read = rx.recv_timeout(DEADLINE);
    let finished = matches!(&read, Ok(Ok(b)) if b.len() as u64 <= MOST_BYTES);
    // the output ended: give the client the rest of the deadline to exit, never more
    let status = loop {
        match child.try_wait() {
            Ok(Some(st)) => break Some(st),
            Ok(None) if finished && Instant::now() < end => std::thread::sleep(Duration::from_millis(10)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                break None;
            }
        }
    };
    match read {
        Err(_) => Err(format!("Tailscale didn't answer in {} s (tailscale status)", DEADLINE.as_secs())),
        Ok(Err(e)) => Err(format!("can't read tailscale's status: {e}")),
        Ok(Ok(b)) if b.len() as u64 > MOST_BYTES => Err("tailscale's status is too large to read".into()),
        Ok(Ok(_)) if status.is_none() => {
            Err(format!("Tailscale didn't finish in {} s (tailscale status)", DEADLINE.as_secs()))
        }
        Ok(Ok(_)) if !status.is_some_and(|s| s.success()) => {
            Err("Tailscale isn't running (tailscale status failed)".into())
        }
        Ok(Ok(b)) => Ok(b),
    }
}

/// The tailnet in `tailscale status --json`'s output. Fields of the wrong type count as missing;
/// a status that isn't an object, or isn't running, is an error.
pub fn parse(bytes: &[u8]) -> Result<Tailnet, String> {
    let bad = |why: &str| format!("can't read tailscale's status: {why}");
    let v: Value = serde_json::from_slice(bytes).map_err(|_| bad("not JSON"))?;
    let state = v.get("BackendState").ok_or_else(|| bad("no BackendState"))?;
    let state = state.as_str().ok_or_else(|| bad("BackendState isn't text"))?;
    if state != "Running" {
        let shown = shown_text(state, 32).to_lowercase();
        return Err(match shown.as_str() {
            "stopped" => "Tailscale is stopped: start it to see your tailnet's machines".into(),
            "needslogin" => "Tailscale needs a login: sign in to see your tailnet's machines".into(),
            _ => format!("Tailscale isn't running (it is {shown})"),
        });
    }
    let text = |v: &Value, k: &str| v.get(k).and_then(Value::as_str).unwrap_or_default().to_string();
    let me = v.get("Self").filter(|s| s.is_object());
    let login = me.and_then(|s| s.get("UserID")).and_then(id).and_then(|uid| {
        let user = v.get("User")?.get(&uid)?;
        Some(shown_text(&text(user, "LoginName"), 128)).filter(|l| !l.is_empty())
    });
    let magic_dns = v.pointer("/CurrentTailnet/MagicDNSEnabled").and_then(Value::as_bool).unwrap_or(true);
    let peers = match v.get("Peer") {
        None | Some(Value::Null) => serde_json::Map::new(),
        Some(Value::Object(p)) => p.clone(),
        Some(_) => return Err(bad("Peer isn't an object")),
    };
    let mut machines: Vec<Machine> = peers
        .values()
        .filter(|p| p.is_object())
        .filter_map(|p| {
            let dns = text(p, "DNSName").trim_end_matches('.').to_string();
            let ips: Vec<String> = match p.get("TailscaleIPs") {
                Some(Value::Array(a)) => {
                    a.iter().filter_map(Value::as_str).filter(|ip| tailnet_ip(ip)).take(8).map(String::from).collect()
                }
                _ => Vec::new(),
            };
            let ip = ips.iter().find(|ip| !ip.contains(':')).or(ips.first()).cloned();
            let ssh = matches!(p.get("sshHostKeys"), Some(Value::Array(k)) if !k.is_empty());
            let online = p.get("Online").and_then(Value::as_bool).unwrap_or(false);
            // an exit node from a location provider (it has a Location) is no machine to log in to
            let provider = p.get("Location").is_some_and(Value::is_object);
            // a DNS name is at most 253 bytes; a longer one is junk, not a machine
            (!provider && !dns.is_empty() && dns.len() <= 253).then(|| Machine {
                dns,
                os: text(p, "OS"),
                online,
                ssh,
                ip,
                ips,
            })
        })
        .collect();
    machines.sort_by(|a, b| b.online.cmp(&a.online).then_with(|| a.dns.cmp(&b.dns)));
    machines.truncate(MOST_PEERS); // after the sort: the online ones are kept
    Ok(Tailnet { login, magic_dns, machines })
}

/// An address inside Tailscale's ranges: 100.64.0.0/10 or fd7a:115c:a1e0::/48. Another one in a
/// peer's list would let it claim a LAN host as a tailnet machine.
fn tailnet_ip(ip: &str) -> bool {
    match ip.parse::<std::net::IpAddr>() {
        Ok(std::net::IpAddr::V4(v4)) => v4.octets()[0] == 100 && v4.octets()[1] & 0xc0 == 64,
        Ok(std::net::IpAddr::V6(v6)) => v6.segments()[..3] == [0xfd7a, 0x115c, 0xa1e0],
        Err(_) => false,
    }
}

/// A user id as the `User` map keys it: the status writes it as a number.
fn id(v: &Value) -> Option<String> {
    match v {
        Value::Number(n) => Some(n.to_string()),
        Value::String(s) if s.bytes().all(|b| b.is_ascii_digit()) => Some(s.clone()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RUNNING: &str = r#"{"BackendState":"Running","Self":{"DNSName":"me.t.ts.net.","UserID":7},
        "User":{"7":{"LoginName":"me@example.com"}},
        "Peer":{"a":{"DNSName":"b.t.ts.net.","OS":"linux","Online":false,"TailscaleIPs":["fd7a:115c:a1e0::2","100.64.0.2"]},
                "b":{"DNSName":"a.t.ts.net.","OS":"macOS","Online":true,"sshHostKeys":["k"]},
                "c":{"DNSName":"","OS":"x","Online":true}, "d": 5,
                "e":{"DNSName":"us-nyc-wg-1.exit.ts.net.","Online":true,"Location":{"Country":"USA"}}}}"#;

    #[test]
    fn reads_peers_online_first_and_who_you_are() {
        let t = parse(RUNNING.as_bytes()).unwrap();
        assert_eq!(t.login.as_deref(), Some("me@example.com"));
        let names: Vec<&str> = t.machines.iter().map(|m| m.dns.as_str()).collect();
        assert_eq!(
            names,
            ["a.t.ts.net", "b.t.ts.net"],
            "online first, no nameless, junk or exit-provider peers, not Self"
        );
        assert!(t.machines[0].ssh && !t.machines[1].ssh);
        assert_eq!(t.machines[1].ip.as_deref(), Some("100.64.0.2"), "IPv4 first");
        assert_eq!(t.machines[0].marks(), "macOS · online · Tailscale SSH");
        let e = t.entry(&t.machines[1], None).unwrap();
        assert_eq!((e.label.as_str(), e.endpoints[0].as_str()), ("b", "ssh://b.t.ts.net"));
        let off = Tailnet { magic_dns: false, ..t.clone() };
        assert_eq!(off.entry(&t.machines[1], None).unwrap().endpoints, ["ssh://100.64.0.2"]);
        let host = Host { label: "x".into(), endpoints: vec!["ssh://me@B.t.ts.net".into()], icon: None };
        assert_eq!(t.machine_for(&host).map(|m| m.dns.as_str()), Some("b.t.ts.net"));
        let by_ip = Host { label: "x".into(), endpoints: vec!["ssh://[fd7a:115c:a1e0::2]".into()], icon: None };
        assert_eq!(t.machine_for(&by_ip).map(|m| m.dns.as_str()), Some("b.t.ts.net"));
    }

    #[test]
    fn junk_is_an_error_not_a_panic() {
        for junk in ["", "null", "[]", "{}", r#"{"BackendState":7}"#, r#"{"BackendState":"Running","Peer":[]}"#] {
            assert!(parse(junk.as_bytes()).is_err(), "{junk}");
        }
        let stopped = parse(br#"{"BackendState":"Stopped","Peer":null}"#).unwrap_err();
        assert!(stopped.contains("stopped"), "{stopped}");
        let weird = parse(b"{\"BackendState\":\"\\u001b[2Jx\"}").unwrap_err();
        assert!(!weird.contains('\u{1b}'), "{weird}");
        // a missing or wrong-typed field is only missing
        let t = parse(br#"{"BackendState":"Running","Self":{"UserID":"x"},"User":3,"Peer":{"a":{"DNSName":"a.b","OS":5,"Online":"yes","sshHostKeys":"k","TailscaleIPs":"1"}}}"#).unwrap();
        assert_eq!(t.login, None);
        assert_eq!(
            t.machines[0],
            Machine { dns: "a.b".into(), os: "".into(), online: false, ssh: false, ip: None, ips: vec![] }
        );
    }

    #[test]
    fn names_that_would_not_pass_the_parsers_are_refused() {
        let t = Tailnet { magic_dns: true, ..Default::default() };
        let m = |dns: &str| Machine { dns: dns.into(), os: "".into(), online: true, ssh: false, ip: None, ips: vec![] };
        for dns in ["a b.t.ts.net", "-oProxyCommand=x.t.ts.net", "a;b.t", &format!("{}.t", "a".repeat(70)), "ü.t"] {
            assert!(t.entry(&m(dns), None).is_err(), "{dns}");
        }
        let off = Tailnet { magic_dns: false, ..Default::default() };
        assert!(off.entry(&m("a.t"), None).is_err(), "no address to use");
        // the status can't pick the ssh user, host or port behind a plain label
        let at = Tailnet { magic_dns: true, ..Default::default() };
        assert!(at.entry(&m("box.t@evil.example"), None).is_err());
        let ip = Machine { ip: Some("root@203.0.113.9".into()), ..m("box.t") };
        assert!(off.entry(&ip, None).is_err());
        let parsed = parse(r#"{"BackendState":"Running","Peer":{"a":{"DNSName":"a.t","TailscaleIPs":["root@1.2.3.4","192.168.1.10","100.64.0.9","fd7a:115c:a1e0::9","fe80::1"]},
            "b":{"DNSName":"LONG"}}}"#.replace("LONG", &"x".repeat(300)).as_bytes()).unwrap();
        assert_eq!(parsed.machines.len(), 1, "a name over 253 bytes is no machine");
        assert_eq!(parsed.machines[0].ips, ["100.64.0.9", "fd7a:115c:a1e0::9"], "only tailnet addresses");
    }

    #[test]
    fn a_short_name_must_be_unique() {
        let m = |dns: &str| Machine { dns: dns.into(), os: "".into(), online: true, ssh: false, ip: None, ips: vec![] };
        let t = Tailnet {
            machines: vec![m("twin.a.ts.net"), m("twin.b.ts.net"), m("solo.a.ts.net")],
            ..Default::default()
        };
        assert!(t.find("twin").unwrap_err().contains("MagicDNS name"));
        assert_eq!(t.find("twin.b.ts.net.").unwrap().dns, "twin.b.ts.net");
        assert_eq!(t.find("SOLO").unwrap().dns, "solo.a.ts.net");
        assert!(t.find("none").is_err());
    }
}
