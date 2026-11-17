//! A fake tailscaled LocalAPI for the serve-over-Tailscale tests: an HTTP/1.0 server on a Unix
//! socket in a private dir that answers `status` and `whois` from JSON the test sets, counts the
//! whois requests, and can be slow or gone. `Serving` starts a leon server pointed at it through
//! the debug-only `LEON_TEST_TAILNET` seam, with 127.0.0.1 standing in for the tailnet. No test
//! talks to a real tailscaled.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixListener;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use super::Env;

pub const OWNER: &str = "owner@example.com";
/// This machine's node, as the status's `Self.ID` names it.
pub const SELF_NODE: &str = "nself";

struct Answers {
    status: Value,
    /// whois's body, or an HTTP status code to answer instead.
    whois: Result<Value, u16>,
    delay: Duration,
}

pub struct Fake {
    pub dir: tempfile::TempDir,
    answers: Arc<Mutex<Answers>>,
    whois_calls: Arc<AtomicUsize>,
    calls: Arc<AtomicUsize>,
}

/// A status as tailscaled's `status?peers=false` gives it: running, signed in as `login`, with `ips`.
pub fn status(login: &str, ips: &[&str]) -> Value {
    json!({ "BackendState": "Running", "Self": { "ID": SELF_NODE, "UserID": 7, "TailscaleIPs": ips, "Tags": null },
            "User": { "7": { "ID": 7, "LoginName": login } } })
}

/// A whois answer for another machine signed in as `login`, with `tags`.
pub fn whois(login: &str, tags: &[&str]) -> Value {
    whois_node("npeer", login, tags)
}

/// A whois answer for the machine `node` (a StableID; `SELF_NODE` is this one).
pub fn whois_node(node: &str, login: &str, tags: &[&str]) -> Value {
    let tags = if tags.is_empty() { Value::Null } else { json!(tags) };
    json!({ "Node": { "ID": 9, "StableID": node, "Name": "peer.example.ts.net.", "User": 8, "Tags": tags },
            "UserProfile": { "ID": 8, "LoginName": login, "DisplayName": "Peer" } })
}

impl Fake {
    /// Signed in as `OWNER` on 127.0.0.1; every peer is the owner, untagged.
    pub fn new() -> Fake {
        let dir = tempfile::Builder::new().permissions(std::fs::Permissions::from_mode(0o700)).tempdir().unwrap();
        let listener = UnixListener::bind(dir.path().join("tailscaled.sock")).unwrap();
        let answers = Arc::new(Mutex::new(Answers {
            status: status(OWNER, &["127.0.0.1"]),
            whois: Ok(whois(OWNER, &[])),
            delay: Duration::ZERO,
        }));
        let (whois_calls, all) = (Arc::new(AtomicUsize::new(0)), Arc::new(AtomicUsize::new(0)));
        let (a, calls, every) = (answers.clone(), whois_calls.clone(), all.clone());
        std::thread::spawn(move || {
            for conn in listener.incoming() {
                let Ok(mut conn) = conn else { return };
                every.fetch_add(1, Ordering::SeqCst);
                let (a, calls) = (a.clone(), calls.clone());
                std::thread::spawn(move || {
                    let mut req = Vec::new();
                    let mut b = [0u8; 1];
                    while !req.ends_with(b"\r\n\r\n") && req.len() < 8192 && conn.read(&mut b).is_ok_and(|n| n == 1) {
                        req.push(b[0]);
                    }
                    let path = String::from_utf8_lossy(&req).split(' ').nth(1).unwrap_or("").to_string();
                    let (body, delay) = {
                        let a = a.lock().unwrap();
                        let body = match path.split('?').next().unwrap_or("") {
                            "/localapi/v0/status" => Ok(a.status.clone()),
                            "/localapi/v0/whois" => {
                                calls.fetch_add(1, Ordering::SeqCst);
                                a.whois.clone()
                            }
                            _ => Err(404),
                        };
                        (body, a.delay)
                    };
                    std::thread::sleep(delay);
                    let reply = match body {
                        Ok(v) => format!("HTTP/1.0 200 OK\r\nContent-Type: application/json\r\n\r\n{v}\n"),
                        Err(code) => format!("HTTP/1.0 {code} Error\r\n\r\nno\n"),
                    };
                    let _ = conn.write_all(reply.as_bytes());
                });
            }
        });
        Fake { dir, answers, whois_calls, calls: all }
    }

    pub fn socket(&self) -> PathBuf {
        self.dir.path().join("tailscaled.sock")
    }

    /// The seam's value: this fake as the LocalAPI, loopback as the tailnet.
    pub fn seam(&self) -> String {
        json!({ "localapi": self.socket(), "loopback": true }).to_string()
    }

    pub fn set_status(&self, v: Value) {
        self.answers.lock().unwrap().status = v;
    }

    pub fn set_whois(&self, v: Result<Value, u16>) {
        self.answers.lock().unwrap().whois = v;
    }

    pub fn set_delay(&self, d: Duration) {
        self.answers.lock().unwrap().delay = d;
    }

    /// tailscaled stopped: its socket is gone.
    pub fn down(&self) {
        std::fs::remove_file(self.socket()).unwrap();
    }

    pub fn whois_calls(&self) -> usize {
        self.whois_calls.load(Ordering::SeqCst)
    }

    /// Connections to the fake, of any kind.
    pub fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

/// A port nothing listens on now.
pub fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port()
}

/// A leon server with `serve_tailscale` on (and `extra` Lua lines), pointed at `fake`.
pub struct Serving {
    pub env: Env,
    pub fake: Fake,
    pub port: u16,
    /// `LEON_TEST_TAILNET` for every command (`Fake::seam` unless a test gives another).
    pub seam: String,
}

impl Serving {
    /// Serving on 127.0.0.1 for the fake's owner: waits until the server says it serves.
    pub fn start(extra: &str) -> Serving {
        let fake = Fake::new();
        let seam = fake.seam();
        let s = Serving::begin(fake, true, extra, seam);
        s.until("serving", |t| t["state"] == "serving");
        s
    }

    /// A started server with `on` for the setting and `seam` in its env; nothing waited for.
    pub fn begin(fake: Fake, on: bool, extra: &str, seam: String) -> Serving {
        let env = Env::new();
        let port = free_port();
        let lua = match on {
            true => {
                format!("leon.set(\"serve_tailscale\", true)\nleon.set(\"serve_tailscale_port\", {port})\n{extra}\n")
            }
            false => format!("{extra}\n"),
        };
        std::fs::write(env.dir.path().join("init.lua"), lua).unwrap();
        let s = Serving { env, fake, port, seam };
        let out = s.cmd().args(["server", "start"]).output().unwrap();
        assert!(out.status.success(), "server start: {}", String::from_utf8_lossy(&out.stderr));
        s
    }

    /// `leon` with this server's config and the seam.
    pub fn cmd(&self) -> Command {
        let mut c = self.env.cmd();
        c.env("LEON_CONFIG", self.env.dir.path().join("init.lua")).env("LEON_TEST_TAILNET", &self.seam);
        c
    }

    /// `server.status`'s `tailnet`.
    pub fn tailnet(&self) -> Value {
        let out = self.cmd().args(["server", "status", "--json"]).output().unwrap();
        assert!(out.status.success(), "server status: {}", String::from_utf8_lossy(&out.stderr));
        let v: Value = serde_json::from_slice(&out.stdout).unwrap();
        v["tailnet"].clone()
    }

    /// Wait up to 10 s for `tailnet` to satisfy `ok`.
    pub fn until(&self, what: &str, ok: impl Fn(&Value) -> bool) -> Value {
        let end = Instant::now() + Duration::from_secs(10);
        loop {
            let t = self.tailnet();
            if ok(&t) {
                return t;
            }
            assert!(Instant::now() < end, "timed out waiting for {what}: {t}");
            std::thread::sleep(Duration::from_millis(100));
        }
    }

    pub fn connect(&self) -> std::io::Result<TcpStream> {
        TcpStream::connect(("127.0.0.1", self.port))
    }

    /// Send `line` and read what comes back until the close (5 s at most).
    pub fn exchange(&self, line: &str) -> Vec<u8> {
        let mut s = self.connect().expect("the listener accepts");
        s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        let _ = s.write_all(line.as_bytes());
        let mut got = Vec::new();
        let mut buf = [0u8; 4096];
        loop {
            match s.read(&mut buf) {
                Ok(0) => return got,
                Ok(n) => got.extend_from_slice(&buf[..n]),
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock || e.kind() == std::io::ErrorKind::TimedOut => {
                    panic!("no close within 5 s; got {:?}", String::from_utf8_lossy(&got))
                }
                Err(_) => return got, // reset: nothing more
            }
        }
    }

    /// A connection that is closed with no reply, even to `server.stop`.
    pub fn assert_refused(&self) {
        let got = self.exchange("{\"id\":1,\"method\":\"server.stop\"}\n");
        assert!(got.is_empty(), "a refused peer gets no reply: {:?}", String::from_utf8_lossy(&got));
        assert!(self.env.socket().exists(), "the server.stop it sent never ran");
        self.tailnet(); // and the server still answers
    }

    /// Turned away: the port refuses the connection, or the server closes it with no reply.
    pub fn assert_turned_away(&self) {
        if self.connect().is_ok() {
            self.assert_refused();
        }
    }

    /// One `server.status` call over TCP: the reply line.
    pub fn status_over_tcp(&self) -> Value {
        self.call(&mut self.connect().expect("the listener accepts"))
    }

    /// A connection that was let in (one `server.status` answered), still open.
    pub fn admitted(&self) -> TcpStream {
        let mut s = self.connect().expect("the listener accepts");
        assert_eq!(self.call(&mut s)["result"]["state"], "running");
        s
    }

    fn call(&self, s: &mut TcpStream) -> Value {
        s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        s.write_all(b"{\"id\":1,\"method\":\"server.status\"}\n").unwrap();
        let mut line = Vec::new();
        let mut b = [0u8; 1];
        while !line.ends_with(b"\n") {
            assert_eq!(s.read(&mut b).expect("a reply within 5 s"), 1, "closed before the reply: {line:?}");
            line.push(b[0]);
        }
        serde_json::from_slice(&line).unwrap()
    }

    pub fn log(&self) -> String {
        std::fs::read_to_string(self.env.dir.path().join("server.log")).unwrap_or_default()
    }
}
