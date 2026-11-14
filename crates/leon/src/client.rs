//! A blocking NDJSON client, with on-demand server start.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use leon_core::ApiError;
use serde_json::{json, Value};

/// Why a command failed, mapped to the spec's exit codes in `main`.
#[derive(Debug)]
pub enum Fail {
    /// The server answered with an error (exit 1, or 3 for target errors).
    Api(ApiError),
    /// No server, and starting one was off or failed (exit 4).
    Unreachable(String),
    /// Anything else (exit 1).
    Other(String),
}

impl From<ApiError> for Fail {
    fn from(e: ApiError) -> Self {
        Fail::Api(e)
    }
}

pub struct Client {
    reader: BufReader<UnixStream>,
    next_id: u64,
}

impl Client {
    pub fn connect(socket: &Path) -> std::io::Result<Self> {
        Ok(Client { reader: BufReader::new(UnixStream::connect(socket)?), next_id: 1 })
    }

    pub fn call(&mut self, method: &str, params: Value) -> Result<Value, Fail> {
        let id = self.next_id;
        self.next_id += 1;
        let mut line = serde_json::to_vec(&json!({ "id": id, "method": method, "params": params })).expect("json");
        line.push(b'\n');
        let io = |e: std::io::Error| Fail::Unreachable(format!("lost the server connection: {e}"));
        self.reader.get_mut().write_all(&line).map_err(io)?;
        let mut reply = String::new();
        if self.reader.read_line(&mut reply).map_err(io)? == 0 {
            return Err(Fail::Unreachable("the server closed the connection".into()));
        }
        let mut v: Value = serde_json::from_str(&reply).map_err(|e| Fail::Other(format!("bad reply: {e}")))?;
        if let Some(err) = v.get("error").filter(|e| !e.is_null()) {
            return Err(Fail::Api(serde_json::from_value(err.clone()).map_err(|e| Fail::Other(e.to_string()))?));
        }
        Ok(v["result"].take())
    }
}

/// Start `leon server run` detached from this process (own session, no inherited stdio), then
/// wait for its socket. Logs go to `server.log` beside the socket.
pub fn spawn_server(socket: &Path, autostarted: bool) -> Result<(), Fail> {
    let log_path = socket.with_file_name("server.log");
    if let Some(dir) = socket.parent() {
        std::fs::create_dir_all(dir).map_err(|e| Fail::Unreachable(format!("can't create {}: {e}", dir.display())))?;
    }
    let log = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .map_err(|e| Fail::Unreachable(format!("can't open {}: {e}", log_path.display())))?;
    let exe = std::env::current_exe().map_err(|e| Fail::Unreachable(e.to_string()))?;
    let mut cmd = Command::new(exe);
    cmd.args(["server", "run"]);
    if autostarted {
        cmd.arg("--autostarted");
    }
    cmd.stdin(Stdio::null()).stdout(log.try_clone().map_err(|e| Fail::Unreachable(e.to_string()))?).stderr(log);
    // SAFETY: setsid is async-signal-safe; it only detaches the child from our terminal/session.
    unsafe {
        cmd.pre_exec(|| {
            libc::setsid();
            Ok(())
        });
    }
    cmd.spawn().map_err(|e| Fail::Unreachable(format!("can't start the server: {e}")))?;
    let end = Instant::now() + Duration::from_secs(5);
    while Instant::now() < end {
        if UnixStream::connect(socket).is_ok() {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    Err(Fail::Unreachable(format!("the server didn't come up; see {}", log_path.display())))
}

/// Connect, starting a personal server first when `autostart` allows.
pub fn connect(socket: &Path, autostart: bool) -> Result<Client, Fail> {
    if let Ok(c) = Client::connect(socket) {
        return Ok(c);
    }
    if !autostart {
        return Err(Fail::Unreachable(format!("no leon server at {}", socket.display())));
    }
    spawn_server(socket, true)?;
    Client::connect(socket).map_err(|e| Fail::Unreachable(format!("can't reach the server: {e}")))
}
