//! The server the app embeds: a `Ctx` served on the usual socket from a background tokio
//! runtime, so the CLI and agents drive the state the app renders straight from memory.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use leon_core::ApiError;
use leon_server::serve::{self, Options};
use leon_server::{Ctx, Info};
use serde_json::{json, Value};

pub struct Embedded {
    ctx: Arc<Ctx>,
    rt: tokio::runtime::Runtime,
    socket: PathBuf,
    /// Set once the socket loop has ended (stopped by us or by `leon server stop`).
    done: Arc<AtomicBool>,
}

/// Serve on the default socket (`LEON_RUNTIME_DIR` etc., docs/architecture.md).
pub fn start() -> Result<Embedded, String> {
    start_at(leon_core::paths::socket_path())
}

/// Serve on `socket`. A live server already there is stopped if it holds no sessions; one with
/// sessions is left alone and the error says how to stop it.
pub fn start_at(socket: PathBuf) -> Result<Embedded, String> {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .thread_name("leon-server")
        .enable_all()
        .build()
        .map_err(|e| e.to_string())?;
    take_over(&socket)?;
    let listener = rt.block_on(serve::listen(&socket)).map_err(|e| match e.kind() {
        std::io::ErrorKind::AddrInUse => busy(&socket, None),
        _ => format!("can't serve {}: {e}", socket.display()),
    })?;
    let info =
        Info { pid: std::process::id(), version: env!("CARGO_PKG_VERSION"), socket: socket.display().to_string() };
    // Not autostarted: the app's server never idle-exits, even with no sessions left.
    let ctx = Arc::new(Ctx::new(info, false));
    let done = Arc::new(AtomicBool::new(false));
    let opts = Options { socket: socket.clone(), idle_exit: Duration::from_secs(10) };
    let (c, d) = (ctx.clone(), done.clone());
    rt.spawn(async move {
        let _ = serve::serve_on(c, listener, opts).await;
        d.store(true, Ordering::SeqCst);
    });
    Ok(Embedded { ctx, rt, socket, done })
}

impl Embedded {
    pub fn ctx(&self) -> &Arc<Ctx> {
        &self.ctx
    }

    /// Run a method in-process. Only for methods that answer at once (not `block.wait`).
    pub fn call(&self, method: &str, params: Value) -> Result<Value, ApiError> {
        self.rt.block_on(self.ctx.call(method, params))
    }

    pub fn stopped(&self) -> bool {
        self.done.load(Ordering::SeqCst)
    }

    /// Stop the way `leon server stop` does (programs end, the socket goes), and wait for it.
    pub fn stop(&self) {
        if !self.stopped() {
            let _ = request(&self.socket, "server.stop", json!({}));
        }
        let end = Instant::now() + Duration::from_secs(5);
        while !self.stopped() && Instant::now() < end {
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}

fn busy(socket: &Path, status: Option<&Value>) -> String {
    let who = match status {
        Some(s) => format!("a leon server (pid {}) with {} session(s)", s["pid"], s["sessions"]),
        None => "another leon server".into(),
    };
    format!("{who} already owns {}; stop it with `leon server stop` and open the app again", socket.display())
}

/// Make the socket ours to bind: nothing answers, or an idle server stopped and exited.
fn take_over(socket: &Path) -> Result<(), String> {
    let Ok(status) = request(socket, "server.status", json!({})) else {
        return Ok(()); // nothing answers; `listen` clears a stale socket
    };
    // The server checks for sessions itself, so one created a moment ago is never lost.
    if request(socket, "server.stop", json!({ "if_idle": true })).is_err() {
        return Err(busy(socket, Some(&status)));
    }
    let pid = status["pid"].as_i64().unwrap_or(0) as i32;
    let end = Instant::now() + Duration::from_secs(10);
    // SAFETY: signal 0 only asks whether the pid exists.
    while pid > 0 && unsafe { libc::kill(pid, 0) } == 0 {
        if Instant::now() > end {
            return Err(busy(socket, Some(&status)));
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    Ok(())
}

/// One NDJSON request on a fresh connection. Err covers "no server" and an error reply alike.
fn request(socket: &Path, method: &str, params: Value) -> Result<Value, String> {
    let mut s = UnixStream::connect(socket).map_err(|e| e.to_string())?;
    s.set_read_timeout(Some(Duration::from_secs(5))).map_err(|e| e.to_string())?;
    let mut line = json!({ "id": 1, "method": method, "params": params }).to_string();
    line.push('\n');
    s.write_all(line.as_bytes()).map_err(|e| e.to_string())?;
    let mut reply = String::new();
    BufReader::new(s).read_line(&mut reply).map_err(|e| e.to_string())?;
    let v: Value = serde_json::from_str(&reply).map_err(|e| e.to_string())?;
    match v.get("result") {
        Some(r) => Ok(r.clone()),
        None => Err(v["error"].to_string()),
    }
}
