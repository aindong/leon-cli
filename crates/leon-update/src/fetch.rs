//! Fetching over HTTPS: the system's `curl`, behind `get` and `download`, so another client can
//! take its place. Nothing here is trusted: the bytes are checked by `manifest` and `install`.
//!
//! The rules (docs/updates.md): HTTPS with curl's certificate checks on; plain HTTP only for a
//! loopback address; every redirect is checked again before it is followed; one plain request
//! with a fixed `User-Agent` and nothing about the machine; a size limit on every body.

use std::io::{Read, Seek, Write};
use std::net::IpAddr;
use std::path::Path;
use std::process::{Command, Stdio};

use crate::{Config, RELEASES};

/// What every request says about its sender: the build's version, and nothing else.
const AGENT: &str = concat!("leon/", env!("CARGO_PKG_VERSION"));
const MAX_REDIRECTS: usize = 5;

/// Fetch a small file into memory: the status of the final answer, and its body.
pub(crate) fn get(cfg: &Config, url: &str, limit: u64, what: &str) -> Result<(u16, Vec<u8>), String> {
    let mut body = Vec::new();
    let status = fetch(cfg, url, limit, what, cfg.timeout * 2, &mut |chunk| {
        match chunk {
            Some(bytes) => body.extend_from_slice(bytes),
            None => body.clear(),
        }
        Ok(())
    })?;
    Ok((status, body))
}

/// Fetch a release file into `to`, reading no more than `limit` bytes of it.
pub(crate) fn download(cfg: &Config, url: &str, limit: u64, what: &str, to: &Path) -> Result<u16, String> {
    let io = |e: std::io::Error| format!("can't write to {}: {e}", to.display());
    let mut file = std::fs::File::create(to).map_err(io)?;
    // an hour at most; a stalled download ends long before, after `timeout` seconds without a byte
    fetch(cfg, url, limit, what, cfg.timeout * 120, &mut |chunk| match chunk {
        Some(bytes) => file.write_all(bytes),
        None => file.set_len(0).and_then(|()| file.rewind()),
    })
    .and_then(|status| file.sync_all().map(|()| status).map_err(io))
}

/// The one function that talks to the network. `sink` gets the body in pieces, and `None` when a
/// redirect means the body starts over. Returns the final HTTP status.
fn fetch(
    cfg: &Config,
    url: &str,
    limit: u64,
    what: &str,
    max_time: u64,
    sink: &mut dyn FnMut(Option<&[u8]>) -> std::io::Result<()>,
) -> Result<u16, String> {
    let mut url = url.to_string();
    let mut began_local = None;
    for _ in 0..=MAX_REDIRECTS {
        let host = allowed(&url)?;
        if !redirect_allowed(*began_local.get_or_insert(host.loopback), &host) {
            return Err(format!(
                "refusing a redirect to {}: a release server may not send leon to this machine's own address",
                url.escape_debug()
            ));
        }
        let mut curl = Command::new("curl");
        // -q first: no ~/.curlrc. No -L: a redirect comes back here to be checked. -N: bytes
        // reach the pipe as they arrive, so what is on disk is what was received. -g: braces and
        // brackets in an address are characters, not curl's patterns for many addresses.
        // -f: an error page is not a body, so it is neither read nor counted against the limit.
        curl.args(["-q", "-sS", "-N", "-g", "-f", "--proto", if host.loopback { "=http" } else { "=https" }])
            .args(["--connect-timeout", &cfg.timeout.to_string(), "--max-time", &max_time.to_string()])
            .args(["--speed-limit", "1", "--speed-time", &cfg.timeout.to_string()])
            // Accept is the one header curl adds that a request can do without
            .args(["-A", AGENT, "-H", "Accept:", "-o", "-", "-w", "%{stderr}\n%{http_code} %{redirect_url}\n"]);
        if host.loopback {
            curl.args(["--noproxy", "*"]); // a proxy is for other machines
        }
        curl.args(["--", &url]).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
        sink(None).map_err(|e| format!("can't write {what}: {e}"))?;
        let mut child = curl.spawn().map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => {
                format!(
                    "updating needs curl, and there is none on PATH; install it, or get the release from {RELEASES}"
                )
            }
            _ => format!("can't run curl: {e}"),
        })?;
        // stderr is read beside stdout: curl writes its last line there before it exits, and a
        // long one (a redirect's address) would fill the pipe while leon waits on the body
        let mut stderr = child.stderr.take().expect("piped");
        let said = std::thread::spawn(move || {
            let mut said = String::new();
            let _ = stderr.read_to_string(&mut said);
            said
        });
        // the limit is enforced here, on the pipe: a server that never stops can't fill the disk
        let (mut body, mut read, mut chunk) = (child.stdout.take().expect("piped"), 0u64, [0u8; 64 << 10]);
        let failed = loop {
            match body.read(&mut chunk) {
                Ok(0) => break None,
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
                Ok(n) if read + n as u64 > limit => {
                    break Some(format!("{what} is larger than the {limit} bytes it may be"))
                }
                Ok(n) => {
                    read += n as u64;
                    if let Err(e) = sink(Some(&chunk[..n])) {
                        break Some(format!("can't write {what}: {e}"));
                    }
                }
                Err(e) => break Some(format!("can't read {what} from curl: {e}")),
            }
        };
        if let Some(failed) = failed {
            let _ = child.kill();
            let _ = child.wait();
            return Err(failed);
        }
        let said = said.join().unwrap_or_default();
        let status = child.wait().map_err(|e| format!("can't wait for curl: {e}"))?;
        // curl's last line is ours: "<status> <where a redirect goes, if this is one>"
        let last = said.lines().last().unwrap_or_default();
        let (code, next) = last.split_once(' ').unwrap_or((last, ""));
        let code = code.parse::<u16>().ok();
        // 22 is -f's exit for an answer of 400 and up: the caller reads the status
        if !status.success() && !(status.code() == Some(22) && code.is_some_and(|c| c >= 400)) {
            return Err(failure(status.code(), &url, &host.name, what, &said, cfg.timeout));
        }
        let code = code.ok_or_else(|| format!("curl gave no status for {url}"))?;
        if !(300..400).contains(&code) || next.is_empty() {
            return Ok(code);
        }
        url = next.to_string();
    }
    Err(format!("too many redirects fetching {what}: the last was to {url}"))
}

/// One line for a curl that failed, from its exit status.
fn failure(code: Option<i32>, url: &str, host: &str, what: &str, said: &str, timeout: u64) -> String {
    let curl = said.lines().find_map(|l| l.strip_prefix("curl: ")).unwrap_or("curl failed").trim();
    match code {
        Some(6) => format!("could not find the host {host}: is this machine online?"),
        Some(7) => format!("could not connect to {host}"),
        Some(18) => format!("the download of {what} ended early"),
        Some(28) => format!("timed out fetching {url} (it stalled for {timeout} s, or took too long)"),
        Some(35) => format!("could not make a secure connection to {host}: {curl}"),
        Some(51 | 58 | 60 | 77 | 83 | 90 | 91) => format!("the certificate of {host} does not check out: {curl}"),
        _ => format!("could not fetch {url}: {curl}"),
    }
}

struct Host {
    /// `host[:port]`, for messages.
    name: String,
    loopback: bool,
}

/// The transport rule, applied to every address before anything connects to it: HTTPS, or plain
/// HTTP to a loopback address given as a number (a name could resolve anywhere).
fn allowed(url: &str) -> Result<Host, String> {
    // escaped: an address that is refused for a control character must not reach the terminal raw
    let refuse = || {
        let shown = url.escape_debug();
        format!("refusing {shown}: releases come over https, and plain http is only for a loopback address")
    };
    let (scheme, rest) = url.split_once("://").ok_or_else(refuse)?;
    let name = rest.split(['/', '?', '#']).next().unwrap_or_default();
    // user@host: what looks like the host is a user name
    if name.is_empty() || name.contains('@') || url.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err(refuse());
    }
    // "[v6]" or "host", then nothing or ":port"
    let (bare, port) = match name.strip_prefix('[').map(|v6| v6.split_once(']')) {
        Some(Some((v6, port))) => (v6, port),
        Some(None) => return Err(refuse()),
        None => name.find(':').map_or((name, ""), |at| name.split_at(at)),
    };
    let port_ok = port.is_empty() || port.strip_prefix(':').is_some_and(|p| p.parse::<u16>().is_ok());
    let loopback = port_ok && bare.parse::<IpAddr>().is_ok_and(|ip| ip.is_loopback());
    match scheme {
        "https" => Ok(Host { name: name.to_string(), loopback: false }),
        "http" if loopback => Ok(Host { name: name.to_string(), loopback: true }),
        _ => Err(refuse()),
    }
}

/// A chain of requests that began at a loopback address (a test, a local mirror) may stay there.
/// One that began anywhere else may never be sent to this machine: a redirect to
/// `http://127.0.0.1:…` would be a request of the server's choosing to a local service.
fn redirect_allowed(began_local: bool, to: &Host) -> bool {
    began_local || !to.loopback
}

#[cfg(test)]
mod tests {
    use super::{allowed, redirect_allowed};

    #[test]
    fn a_release_server_cannot_redirect_to_this_machine() {
        let (local, github) = (allowed("http://127.0.0.1:8080/x").unwrap(), allowed("https://github.com/x").unwrap());
        assert!(redirect_allowed(true, &local), "loopback to loopback: the tests, a local mirror");
        assert!(redirect_allowed(true, &github));
        assert!(redirect_allowed(false, &github), "https to https: what GitHub does");
        assert!(!redirect_allowed(false, &local), "https to loopback http");
        assert!(!allowed("https://127.0.0.1/x").unwrap().loopback, "an https loopback address is not the exception");
    }

    #[test]
    fn only_https_and_loopback_http_are_allowed() {
        for ok in ["https://github.com/x", "https://127.0.0.1:8443/x", "http://127.0.0.1:8080/x", "http://[::1]:8080/x"]
        {
            assert!(allowed(ok).is_ok(), "{ok}");
        }
        assert!(allowed("http://127.0.0.1/x").unwrap().loopback);
        assert!(!allowed("https://127.0.0.1/x").unwrap().loopback, "https is never the loopback exception");
        for bad in [
            "http://example.com/x",
            "http://localhost/x", // a name: it resolves wherever the resolver says
            "http://127.0.0.1.example.com/x",
            "http://127.0.0.1@example.com/x",
            "https://user@example.com/x",
            "http://[::1].example.com/x",
            "http://2130706433/x", // 127.0.0.1 as one number: curl would take it, IpAddr doesn't
            "ftp://127.0.0.1/x",
            "file:///tmp/x",
            "127.0.0.1/x",
            "https:///x",
            "https://example.com/x y",
            "",
        ] {
            assert!(allowed(bad).is_err(), "{bad}");
        }
    }
}
