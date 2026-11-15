//! A blocking NDJSON client, with on-demand server start.

use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::net::UnixStream;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use leon_core::ApiError;
use leon_hosts::Link;
use serde_json::{json, Value};

/// Replies longer than this are refused, like requests are on the server.
pub(crate) const MAX_REPLY: u64 = 64 << 20;

/// Why a command failed, mapped to the spec's exit codes in `main`.
#[derive(Debug)]
pub enum Fail {
    /// The server answered with an error (exit 1, or 3 for target errors).
    Api(ApiError),
    /// No server, and starting one was off or failed (exit 4).
    Unreachable(String),
    /// Anything else (exit 1).
    Other(String),
    /// A bad command line that clap can't see, such as a bad -H endpoint (exit 2).
    Usage(String),
    /// Exit quietly with this status (`wait` passing on a program's exit status).
    Exit(i32),
}

impl From<ApiError> for Fail {
    fn from(e: ApiError) -> Self {
        Fail::Api(e)
    }
}

pub struct Client {
    reader: BufReader<UnixStream>,
    next_id: u64,
    /// Pushed event lines that arrived while waiting for a reply.
    events: VecDeque<Value>,
}

impl Client {
    /// Connect, but only to a socket in a private dir (the server binds only in one): a socket in a
    /// dir others can write to could be anyone's, and the app types into what answers.
    pub fn connect(socket: &Path) -> std::io::Result<Self> {
        leon_core::paths::ensure_private_dir(socket.parent().unwrap_or(Path::new(".")))?;
        Ok(Client { reader: BufReader::new(UnixStream::connect(socket)?), next_id: 1, events: VecDeque::new() })
    }

    /// Give up on a reply (or a write) after `limit`, as an unreachable server; None waits forever.
    pub fn set_timeout(&self, limit: Option<Duration>) -> std::io::Result<()> {
        let stream = self.reader.get_ref();
        stream.set_read_timeout(limit)?;
        stream.set_write_timeout(limit)
    }

    /// The next line the server sends, parsed; None when it closed the connection.
    fn read(&mut self) -> Result<Option<Value>, Fail> {
        let io = |e: std::io::Error| Fail::Unreachable(format!("lost the server connection: {e}"));
        let mut line = Vec::new();
        let n = (&mut self.reader).take(MAX_REPLY + 1).read_until(b'\n', &mut line).map_err(io)?;
        if n == 0 {
            return Ok(None);
        }
        if line.len() as u64 > MAX_REPLY {
            return Err(Fail::Other("the server's reply is too large".into()));
        }
        serde_json::from_slice(&line).map(Some).map_err(|e| Fail::Other(format!("bad reply: {e}")))
    }

    /// The next pushed event line (`{"event":…,"seq":N}`) after an attach.
    pub fn next_event(&mut self) -> Result<Value, Fail> {
        if let Some(e) = self.events.pop_front() {
            return Ok(e);
        }
        let behind = leon_server::events::MAX_BACKLOG >> 20;
        self.read()?.ok_or_else(|| {
            Fail::Other(format!(
                "the server closed the event stream (it drops a reader that falls {behind} MiB behind)"
            ))
        })
    }

    pub fn call(&mut self, method: &str, params: Value) -> Result<Value, Fail> {
        let id = self.next_id;
        self.next_id += 1;
        let mut line = serde_json::to_vec(&json!({ "id": id, "method": method, "params": params })).expect("json");
        line.push(b'\n');
        let io = |e: std::io::Error| Fail::Unreachable(format!("lost the server connection: {e}"));
        self.reader.get_mut().write_all(&line).map_err(io)?;
        let mut v = loop {
            match self.read()? {
                None => return Err(Fail::Unreachable("the server closed the connection".into())),
                Some(v) if v.get("event").is_some() => self.events.push_back(v),
                Some(v) => break v,
            }
        };
        if let Some(err) = v.get("error").filter(|e| !e.is_null()) {
            return Err(Fail::Api(serde_json::from_value(err.clone()).map_err(|e| Fail::Other(e.to_string()))?));
        }
        Ok(v["result"].take())
    }
}

/// Start `leon server run` detached from this process (own session, no inherited stdio, cwd `/`),
/// then wait for its socket. Logs go to `server.log` beside the socket.
pub fn spawn_server(socket: &Path, autostarted: bool, restore: bool) -> Result<(), Fail> {
    let log_path = socket.with_file_name("server.log");
    leon_core::paths::check_socket_path(socket).map_err(|e| Fail::Unreachable(e.to_string()))?;
    if let Some(dir) = socket.parent() {
        leon_core::paths::ensure_private_dir(dir).map_err(|e| Fail::Unreachable(e.to_string()))?;
    }
    let log = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(&log_path)
        .map_err(|e| Fail::Unreachable(format!("can't open {}: {e}", log_path.display())))?;
    let exe = std::env::current_exe().map_err(|e| Fail::Unreachable(e.to_string()))?;
    let mut cmd = Command::new(exe);
    cmd.args(["server", "run"]);
    if autostarted {
        cmd.arg("--autostarted");
    }
    if !restore {
        cmd.arg("--restore=false");
    }
    // The server runs in `/`, so a relative state dir means one under this directory.
    if let Some(dir) = std::env::var_os("LEON_STATE_DIR").map(std::path::PathBuf::from).filter(|d| d.is_relative()) {
        if let (false, Ok(here)) = (dir.as_os_str().is_empty(), std::env::current_dir()) {
            cmd.env("LEON_STATE_DIR", here.join(dir));
        }
    }
    cmd.current_dir("/")
        .stdin(Stdio::null())
        .stdout(log.try_clone().map_err(|e| Fail::Unreachable(e.to_string()))?)
        .stderr(log);
    // SAFETY: setsid is async-signal-safe; it only detaches the child from our terminal/session.
    unsafe {
        cmd.pre_exec(|| {
            libc::setsid();
            Ok(())
        });
    }
    let mut child = cmd.spawn().map_err(|e| Fail::Unreachable(format!("can't start the server: {e}")))?;
    let end = Instant::now() + Duration::from_secs(5);
    while Instant::now() < end {
        if UnixStream::connect(socket).is_ok() {
            return Ok(());
        }
        // It died (bad runtime dir, or another server won the race): stop waiting.
        if child.try_wait().ok().flatten().is_some() && UnixStream::connect(socket).is_err() {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    Err(Fail::Unreachable(format!("the server didn't come up; see {}", log_path.display())))
}

/// Where client commands go: this machine's socket, or another machine's through a `Link`.
pub struct Server {
    pub socket: PathBuf,
    /// The host's name in messages, and the link (kept open while the command runs).
    pub link: Option<(String, Link)>,
}

impl Server {
    pub fn local() -> Server {
        Server { socket: leon_core::paths::socket_path(), link: None }
    }

    pub fn over(name: &str, link: Link) -> Server {
        Server { socket: link.socket().to_path_buf(), link: Some((name.to_string(), link)) }
    }

    /// A checked connection. Locally a server may be started (`autostart`); over a link the
    /// host's proxy decides that, as the link was opened to.
    pub fn connect(&self, autostart: bool) -> Result<Client, Fail> {
        let Some((name, _)) = &self.link else { return connect(&self.socket, autostart) };
        connect(&self.socket, false).map_err(|f| match f {
            Fail::Unreachable(_) => Fail::Unreachable(format!("can't reach leon on {name}")),
            f => f,
        })
    }
}

/// Connect, starting a personal server first when `autostart` allows. The connection is checked
/// with a ping: a server that was idle-exiting can accept a connection and then close it.
pub fn connect(socket: &Path, autostart: bool) -> Result<Client, Fail> {
    if let Ok(mut c) = Client::connect(socket) {
        match c.call("server.version", json!({})) {
            Ok(_) => return Ok(c),
            Err(Fail::Unreachable(_)) => {} // gone between connect and reply: treat as no server
            Err(f) => return Err(f),
        }
    }
    if !autostart {
        return Err(Fail::Unreachable(format!("no leon server at {}", socket.display())));
    }
    spawn_server(socket, true, true)?;
    Client::connect(socket).map_err(|e| Fail::Unreachable(format!("can't reach the server: {e}")))
}
