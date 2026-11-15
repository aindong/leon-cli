//! Other machines' leon servers (docs/architecture.md, "Remote hosts"): an `Endpoint` such as
//! `ssh://me@devbox:2222`, the hosts list that names them, and the `Link` that reaches one through
//! the system ssh. Used by the CLI's `-H` and, later, by the app.

pub mod hosts;
mod link;

use std::process::Command;
use std::str::FromStr;

pub use hosts::Host;
pub use link::Link;

/// Copy `from` into `to` until `from` ends or either side fails. Not `std::io::copy`: on Linux it
/// splices, and a splice from a socket into a pipe holds the pipe locked while it waits for data,
/// so the process at the pipe's other end (sshd, ssh) can't even do a non-blocking read of it.
pub fn pump(from: &mut impl std::io::Read, to: &mut impl std::io::Write) {
    let mut buf = vec![0; 64 << 10];
    loop {
        match from.read(&mut buf) {
            Ok(0) => return,
            Ok(n) if to.write_all(&buf[..n]).and_then(|()| to.flush()).is_ok() => {}
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            _ => return,
        }
    }
}

/// `ssh://[USER@]HOST[:PORT]`. Parsing is the guard: every part is a plain word that can't start
/// with `-`, so nothing in an endpoint reaches ssh as an option or a shell as code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Endpoint {
    pub user: Option<String>,
    /// A name (`devbox`, an ~/.ssh/config alias, an IPv4 address) or an IPv6 address without brackets.
    pub host: String,
    pub port: Option<u16>,
}

/// A user or host name: letters, digits, `.`, `_`, `-`, not starting with `-`.
fn word(s: &str, max: usize) -> bool {
    !s.is_empty()
        && s.len() <= max
        && !s.starts_with('-')
        && s.bytes().all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
}

impl FromStr for Endpoint {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, String> {
        let bad = |why: &str| format!("bad endpoint {s:?}: {why} (expected ssh://[USER@]HOST[:PORT])");
        let rest = s.strip_prefix("ssh://").ok_or_else(|| bad("it must start with ssh://"))?;
        let (user, rest) = match rest.split_once('@') {
            Some((u, r)) if word(u, 64) => (Some(u.to_string()), r),
            Some(_) => return Err(bad("the user name may only have letters, digits, . _ - and can't start with -")),
            None => (None, rest),
        };
        let (host, port) = match rest.strip_prefix('[') {
            Some(v6) => {
                let (addr, after) = v6.split_once(']').ok_or_else(|| bad("an unclosed ["))?;
                let ok = !addr.is_empty()
                    && addr.len() <= 45
                    && addr.bytes().all(|b| b.is_ascii_hexdigit() || b":.".contains(&b));
                if !ok {
                    return Err(bad("not an IPv6 address inside [ ]"));
                }
                match after {
                    "" => (addr, None),
                    _ => (addr, Some(after.strip_prefix(':').ok_or_else(|| bad("junk after ]"))?)),
                }
            }
            None => match rest.split_once(':') {
                Some((h, p)) => (h, Some(p)),
                None => (rest, None),
            },
        };
        if !host.contains(':') && !word(host, 253) {
            return Err(bad("the host may only have letters, digits, . _ - and can't start with -"));
        }
        let port = match port {
            None => None,
            Some(p) if !p.is_empty() && p.len() <= 5 && p.bytes().all(|b| b.is_ascii_digit()) => {
                Some(p.parse().ok().filter(|&n: &u16| n > 0).ok_or_else(|| bad("the port must be 1 to 65535"))?)
            }
            Some(_) => return Err(bad("the port must be 1 to 65535")),
        };
        Ok(Endpoint { user, host: host.to_string(), port })
    }
}

impl std::fmt::Display for Endpoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ssh://")?;
        if let Some(u) = &self.user {
            write!(f, "{u}@")?;
        }
        match self.host.contains(':') {
            true => write!(f, "[{}]", self.host)?,
            false => write!(f, "{}", self.host)?,
        }
        if let Some(p) = self.port {
            write!(f, ":{p}")?;
        }
        Ok(())
    }
}

/// Text a host sent (ssh's stderr, a server's error) as the app may show it: no control or bidi
/// characters, at most `max` characters, trimmed.
pub fn shown_text(text: &str, max: usize) -> String {
    let bidi =
        |c: char| matches!(c, '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}' | '\u{200e}' | '\u{200f}' | '\u{061c}');
    let kept: String = text.trim().chars().filter(|&c| !c.is_control() && !bidi(c)).take(max).collect();
    kept.trim().to_string()
}

/// What ssh runs on the host: a fixed string with nothing from the endpoint in it. It finds
/// `leon` on PATH, else in ~/.local/bin (where scripts/install.sh links it), else exits 127. Plain
/// `sh -c '…'` reads the same under sh, bash, zsh and fish login shells.
pub fn remote_command(autostart: bool) -> String {
    let proxy = if autostart { "server proxy" } else { "server proxy --autostart=false" };
    format!(
        "sh -c 'if command -v leon >/dev/null 2>&1; then exec leon {proxy}; \
         elif [ -x \"$HOME/.local/bin/leon\" ]; then exec \"$HOME/.local/bin/leon\" {proxy}; else exit 127; fi'"
    )
}

impl Endpoint {
    /// ssh's arguments. leon adds `-T` (no tty: the pipe carries NDJSON), no forwarding of any kind
    /// (the link needs none: no agent on the host, no port bound by a config's LocalForward), and
    /// keepalives, so a dead network is noticed in about 45 s, and a connect timeout, so a command
    /// started during an outage fails in 15 s instead of waiting it out (a stalled banner counts);
    /// the rest comes from ~/.ssh/config.
    pub fn ssh_args(&self, autostart: bool) -> Vec<String> {
        let mut args: Vec<String> = [
            "-T",
            "-a",
            "-x",
            "-o",
            "ClearAllForwardings=yes",
            "-o",
            "ServerAliveInterval=15",
            "-o",
            "ServerAliveCountMax=3",
            "-o",
            "ConnectTimeout=15",
        ]
        .map(String::from)
        .to_vec();
        if let Some(p) = self.port {
            args.extend(["-p".into(), p.to_string()]);
        }
        let dest = match &self.user {
            Some(u) => format!("{u}@{}", self.host),
            None => self.host.clone(),
        };
        args.extend(["--".into(), dest, remote_command(autostart)]);
        args
    }

    /// `$LEON_SSH` (default `ssh`) with `ssh_args`, run directly, never through a local shell.
    /// With `batch`, ssh fails instead of asking for a password or passphrase.
    pub fn ssh(&self, autostart: bool, batch: bool) -> Command {
        let program = std::env::var_os("LEON_SSH").filter(|p| !p.is_empty()).unwrap_or_else(|| "ssh".into());
        let mut args = self.ssh_args(autostart);
        if batch {
            let at = args.iter().position(|a| a == "--").expect("ssh_args has --");
            args.splice(at..at, ["-o".to_string(), "BatchMode=yes".to_string()]);
        }
        let mut c = Command::new(program);
        c.args(args);
        c
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn good_endpoints_round_trip() {
        for s in [
            "ssh://box",
            "ssh://me@box",
            "ssh://me@box.example.com:2222",
            "ssh://10.0.0.2:22",
            "ssh://[::1]:2200",
            "ssh://u_1@[fe80::1]",
        ] {
            let e: Endpoint = s.parse().unwrap();
            assert_eq!(e.to_string(), s);
        }
        let e: Endpoint = "ssh://me@[::1]:2200".parse().unwrap();
        assert_eq!((e.user.as_deref(), e.host.as_str(), e.port), (Some("me"), "::1", Some(2200)));
    }

    #[test]
    fn bad_endpoints_are_refused() {
        for s in [
            "box",
            "ssh://",
            "ssh://-oProxyCommand=x",
            "ssh://a b",
            "ssh://a;b",
            "ssh://u@-x",
            "ssh://-u@x",
            "ssh://a:0",
            "ssh://a:65536",
            "ssh://a:+22",
            "ssh://a:",
            "ssh://a/s",
            "ssh://[::1",
            "ssh://[x]",
            "ssh://[::1]x",
            "ssh://a@b@c",
            "ssh://ü",
            "ssh://a\nb",
            "ssh://@a",
        ] {
            assert!(s.parse::<Endpoint>().is_err(), "{s:?} was accepted");
        }
    }

    #[test]
    fn the_host_goes_after_double_dash() {
        let e: Endpoint = "ssh://me@box:2222".parse().unwrap();
        let a = e.ssh_args(true);
        let at = a.iter().position(|x| x == "--").unwrap();
        assert_eq!(&a[at + 1..at + 2], ["me@box"]);
        assert_eq!(a.len(), at + 3);
        assert!(!remote_command(true).contains("autostart"));
        assert!(remote_command(false).contains("--autostart=false"));
    }
}
