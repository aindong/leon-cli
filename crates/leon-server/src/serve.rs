//! The socket server: NDJSON requests in, NDJSON responses out (docs/architecture.md).

use std::io;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use leon_core::ApiError;
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::Notify;

use crate::{Ctx, Info};

pub struct Options {
    pub socket: PathBuf,
    /// Started on demand by a client: exit after `idle_exit` with no sessions and no connections.
    pub autostarted: bool,
    pub idle_exit: Duration,
}

#[derive(Deserialize)]
struct Request {
    #[serde(default)]
    id: Value,
    method: String,
    #[serde(default)]
    params: Value,
}

struct Shared {
    ctx: Ctx,
    open: AtomicUsize,
    last_activity: Mutex<Instant>,
    shutdown: Notify,
}

impl Shared {
    fn touch(&self) {
        *self.last_activity.lock().unwrap_or_else(|e| e.into_inner()) = Instant::now();
    }
}

/// Bind the socket (refusing if a live server already owns it), then serve until stopped.
pub async fn serve(opts: Options) -> io::Result<()> {
    let listener = bind(&opts.socket).await?;
    let info = Info {
        pid: std::process::id(),
        version: env!("CARGO_PKG_VERSION"),
        socket: opts.socket.display().to_string(),
        autostarted: opts.autostarted,
    };
    let shared = Arc::new(Shared {
        ctx: Ctx::new(info),
        open: AtomicUsize::new(0),
        last_activity: Mutex::new(Instant::now()),
        shutdown: Notify::new(),
    });
    let mut tick = tokio::time::interval(Duration::from_millis(200));
    loop {
        tokio::select! {
            accepted = listener.accept() => {
                let Ok((stream, _)) = accepted else { continue };
                shared.open.fetch_add(1, Ordering::SeqCst);
                shared.touch();
                let s = shared.clone();
                tokio::spawn(async move {
                    let _ = connection(stream, &s).await;
                    s.open.fetch_sub(1, Ordering::SeqCst);
                    s.touch();
                });
            }
            _ = shared.shutdown.notified() => break,
            _ = tick.tick(), if opts.autostarted => {
                let idle = shared.open.load(Ordering::SeqCst) == 0
                    && shared.ctx.state.lock().unwrap_or_else(|e| e.into_inner()).sessions.is_empty()
                    && shared.last_activity.lock().unwrap_or_else(|e| e.into_inner()).elapsed() >= opts.idle_exit;
                if idle { break; }
            }
        }
    }
    let _ = std::fs::remove_file(&opts.socket);
    Ok(())
}

async fn bind(socket: &PathBuf) -> io::Result<UnixListener> {
    // Make a runtime dir we create private, but never chmod one that already exists:
    // LEON_RUNTIME_DIR can name a shared directory such as /tmp.
    if let Some(dir) = socket.parent().filter(|d| !d.exists()) {
        std::fs::create_dir_all(dir)?;
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?;
    }
    if socket.exists() {
        if UnixStream::connect(socket).await.is_ok() {
            return Err(io::Error::new(
                io::ErrorKind::AddrInUse,
                format!("a leon server already listens on {}", socket.display()),
            ));
        }
        std::fs::remove_file(socket)?; // stale socket from a server that died
    }
    let listener = UnixListener::bind(socket)?;
    std::fs::set_permissions(socket, std::fs::Permissions::from_mode(0o600))?;
    Ok(listener)
}

async fn connection(stream: UnixStream, shared: &Shared) -> io::Result<()> {
    let (read, mut write) = stream.into_split();
    let mut lines = BufReader::new(read).lines();
    while let Some(line) = lines.next_line().await? {
        if line.trim().is_empty() {
            continue;
        }
        shared.touch();
        let reply = match serde_json::from_str::<Request>(&line) {
            Ok(req) => match shared.ctx.call(&req.method, req.params) {
                Ok(result) => json!({ "id": req.id, "result": result }),
                Err(e) => json!({ "id": req.id, "error": e }),
            },
            Err(e) => json!({ "id": null, "error": ApiError::bad_request(format!("not a request: {e}")) }),
        };
        let mut out = serde_json::to_vec(&reply)?;
        out.push(b'\n');
        write.write_all(&out).await?;
        write.flush().await?;
        if shared.ctx.stopping.load(Ordering::SeqCst) {
            shared.shutdown.notify_one(); // only after the reply to `server.stop` is out
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn bind_leaves_an_existing_dirs_mode_alone() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o755)).unwrap();
        let socket = dir.path().join("server.sock");
        let _l = bind(&socket).await.unwrap();
        let mode = |p: &std::path::Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(dir.path()), 0o755, "LEON_RUNTIME_DIR=/tmp must not chmod /tmp");
        assert_eq!(mode(&socket), 0o600);
    }

    #[tokio::test]
    async fn bind_makes_a_new_dir_private() {
        let parent = tempfile::tempdir().unwrap();
        let socket = parent.path().join("leon/server.sock");
        let _l = bind(&socket).await.unwrap();
        let mode = std::fs::metadata(parent.path().join("leon")).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o700);
    }
}
