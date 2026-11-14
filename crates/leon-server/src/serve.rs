//! The socket server: NDJSON requests in, NDJSON responses out, and pushed event lines on
//! connections that attached (docs/architecture.md).
//!
//! Access control is the private runtime dir (leon_core::paths::ensure_private_dir) plus a peer
//! uid check on every connection: once terminals exist, a connection can run code as the user.

use std::io;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use leon_core::ApiError;
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::unix::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::{UnixListener, UnixStream};
use tokio::signal::unix::{signal, SignalKind};
use tokio::sync::{mpsc, Notify};

use crate::events::{is_attach, Subscriber};
use crate::persist::{self, Persist};
use crate::{Ctx, Later};

/// Event lines written between two looks at replies and requests.
const EVENT_BATCH: usize = 64;

/// The longest request line accepted (1 MiB); a longer one gets an error and the connection closes.
pub const MAX_LINE: usize = 1 << 20;

pub struct Options {
    pub socket: PathBuf,
    /// An autostarted server (`Ctx::is_autostarted`) exits after this long with no sessions and no connections.
    pub idle_exit: Duration,
    /// Restore sessions on start and save them on stop; None keeps nothing.
    pub persist: Option<Persist>,
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
    ctx: Arc<Ctx>,
    open: AtomicUsize,
    last_activity: Mutex<Instant>,
    shutdown: Notify,
    /// Set by `server.stop {discard: true}`.
    discard: AtomicBool,
}

impl Shared {
    fn touch(&self) {
        *self.last_activity.lock().unwrap_or_else(|e| e.into_inner()) = Instant::now();
    }
}

/// Counts an open connection for the idle check; the drop also covers a panicking handler.
struct Open(Arc<Shared>);

impl Drop for Open {
    fn drop(&mut self) {
        self.0.touch(); // before the decrement, so the idle clock never sees 0 open with a stale time
        self.0.open.fetch_sub(1, Ordering::SeqCst);
    }
}

/// Bind the socket (refusing if a live server already owns it), restore the saved sessions, then
/// serve `ctx` until stopped (`server.stop`, SIGTERM, SIGINT, SIGHUP, or an autostarted server's idle exit),
/// and save the sessions before every program ends.
pub async fn serve(ctx: Arc<Ctx>, opts: Options) -> io::Result<()> {
    let (listener, ours) = bind(&opts.socket).await?;
    // Before restoring, so a signal from here on saves instead of killing the process.
    let mut term = signal(SignalKind::terminate())?;
    let mut int = signal(SignalKind::interrupt())?;
    let mut hup = signal(SignalKind::hangup())?; // `server run`'s terminal closed
    let (state_file, _state_lock) = opts.persist.as_ref().and_then(|p| persist::start(&ctx, p)).unzip();
    *crate::events::lock(&ctx.state_file) = state_file.clone();
    let shared = Arc::new(Shared {
        ctx,
        open: AtomicUsize::new(0),
        last_activity: Mutex::new(Instant::now()),
        shutdown: Notify::new(),
        discard: AtomicBool::new(false),
    });
    let mut tick = tokio::time::interval(Duration::from_millis(200));
    loop {
        tokio::select! {
            accepted = listener.accept() => {
                let stream = match accepted {
                    Ok((stream, _)) => stream,
                    Err(_) => { // e.g. out of file descriptors: don't spin
                        tokio::time::sleep(Duration::from_millis(50)).await;
                        continue;
                    }
                };
                if !same_user(&stream) {
                    continue; // dropping the stream closes it
                }
                shared.open.fetch_add(1, Ordering::SeqCst);
                shared.touch();
                let guard = Open(shared.clone());
                tokio::spawn(async move {
                    let _ = connection(stream, &guard.0).await;
                    drop(guard);
                });
            }
            _ = shared.shutdown.notified() => break,
            _ = term.recv() => break,
            _ = int.recv() => break,
            _ = hup.recv() => break,

            _ = tick.tick() => {
                let idle = shared.ctx.is_autostarted()
                    && shared.open.load(Ordering::SeqCst) == 0
                    && shared.ctx.state().sessions.is_empty()
                    && shared.last_activity.lock().unwrap_or_else(|e| e.into_inner()).elapsed() >= opts.idle_exit;
                if idle { break; }
            }
        }
    }
    if let Some(file) = &state_file {
        let done = match shared.discard.load(Ordering::SeqCst) {
            true => persist::discard(file),
            false => persist::save(&shared.ctx.state(), file),
        };
        if let Err(e) = done {
            eprintln!("leon: warning: can't save sessions to {}: {e}", file.display());
        }
    }
    shared.ctx.end_programs(Duration::from_secs(2)).await;
    // Only remove the socket if the path is still ours; a newer server may have bound it.
    if std::fs::symlink_metadata(&opts.socket).is_ok_and(|m| (m.dev(), m.ino()) == ours) {
        let _ = std::fs::remove_file(&opts.socket);
    }
    Ok(())
}

fn same_user(stream: &UnixStream) -> bool {
    // SAFETY: geteuid has no preconditions.
    stream.peer_cred().is_ok_and(|c| c.uid() == unsafe { libc::geteuid() })
}

/// Bind in a private runtime dir. Returns the listener and the socket's (dev, inode). Holds an
/// exclusive lock beside the socket so two servers starting at once can't both decide a socket
/// is stale and unlink each other's.
async fn bind(socket: &Path) -> io::Result<(UnixListener, (u64, u64))> {
    let dir = socket.parent().unwrap_or(Path::new("."));
    leon_core::paths::ensure_private_dir(dir)?;
    let _lock = lock(&dir.join("server.lock"))?;
    if std::fs::symlink_metadata(socket).is_ok() {
        match UnixStream::connect(socket).await {
            Ok(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::AddrInUse,
                    format!("a leon server already listens on {}", socket.display()),
                ))
            }
            // Only a refused connection means stale; anything else may be a live server we can't reach.
            Err(e) if e.kind() == io::ErrorKind::ConnectionRefused => std::fs::remove_file(socket)?,
            Err(e) => return Err(io::Error::new(e.kind(), format!("{} is in use: {e}", socket.display()))),
        }
    }
    let listener = UnixListener::bind(socket)?;
    std::fs::set_permissions(socket, std::fs::Permissions::from_mode(0o600))?;
    let m = std::fs::symlink_metadata(socket)?;
    Ok((listener, (m.dev(), m.ino())))
}

/// An exclusive flock on `path`, released when the returned file drops.
fn lock(path: &Path) -> io::Result<std::fs::File> {
    use std::os::unix::fs::OpenOptionsExt;
    use std::os::unix::io::AsRawFd;
    let f = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)?;
    // SAFETY: flock on a file descriptor we own.
    if unsafe { libc::flock(f.as_raw_fd(), libc::LOCK_EX) } != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(f)
}

/// One request line from the client, or the end of a line too long to read.
enum Line {
    Text(Vec<u8>),
    TooLarge,
}

/// One connection: requests are answered one at a time, in order, while events the connection
/// attached to are written between replies.
async fn connection(stream: UnixStream, shared: &Shared) -> io::Result<()> {
    let (read, mut write) = stream.into_split();
    let (tx, mut lines) = mpsc::channel(1);
    let reader = tokio::spawn(read_lines(read, tx));
    let sub = Arc::new(Subscriber::default());
    let result = serve_lines(&mut lines, &mut write, &sub, shared).await;
    reader.abort();
    result
}

async fn read_lines(read: OwnedReadHalf, tx: mpsc::Sender<Line>) -> io::Result<()> {
    let mut reader = BufReader::new(read);
    loop {
        let mut line = Vec::new();
        let n = (&mut reader).take(MAX_LINE as u64 + 1).read_until(b'\n', &mut line).await?;
        if n == 0 {
            return Ok(());
        }
        // Stop at an oversized line: the rest of it is never read.
        let too_large = line.len() > MAX_LINE;
        let msg = if too_large { Line::TooLarge } else { Line::Text(line) };
        if tx.send(msg).await.is_err() || too_large {
            return Ok(());
        }
    }
}

async fn serve_lines(
    lines: &mut mpsc::Receiver<Line>,
    write: &mut OwnedWriteHalf,
    sub: &Arc<Subscriber>,
    shared: &Shared,
) -> io::Result<()> {
    // Order: a finished call's reply, then the next request, then events, so an event flood can't
    // hold up replies and an attach reply precedes its events.
    // The call being answered: its id, Some(discard) if it stops the server, and its result.
    let mut pending: Option<(Value, Option<bool>, Later<Value>)> = None;
    loop {
        tokio::select! {
            biased;
            result = async { pending.as_mut().expect("guarded").2.as_mut().await }, if pending.is_some() => {
                let (id, stop, _) = pending.take().expect("guarded");
                let stop = stop.filter(|_| result.is_ok());
                let reply = match result {
                    Ok(result) => json!({ "id": id, "result": result }),
                    Err(e) => json!({ "id": id, "error": e }),
                };
                if !send(write, &line(&reply), sub).await? {
                    return Ok(());
                }
                if let Some(discard) = stop {
                    shared.discard.store(discard, Ordering::SeqCst);
                    shared.shutdown.notify_one(); // only after this connection's own stop reply is out
                }
            }
            msg = lines.recv(), if pending.is_none() => {
                let text = match msg {
                    None => return Ok(()),
                    Some(Line::TooLarge) => {
                        let e = ApiError::new(413, "request_too_large", format!("requests are at most {MAX_LINE} bytes"));
                        send(write, &line(&json!({ "id": null, "error": e })), sub).await?;
                        return Ok(()); // close: the rest of the oversized line is never read
                    }
                    Some(Line::Text(text)) => text,
                };
                let text = String::from_utf8_lossy(&text);
                if text.trim().is_empty() {
                    continue;
                }
                shared.touch();
                match serde_json::from_str::<Request>(&text) {
                    Ok(req) => {
                        // The handler checks the params; a stop's `discard` is read here.
                        let stop = (req.method == "server.stop")
                            .then(|| req.params.get("discard").and_then(Value::as_bool).unwrap_or(false));
                        let later: Later<Value> = if is_attach(&req.method) {
                            // Here, not in the registry: attaching subscribes this connection.
                            Box::pin(std::future::ready(shared.ctx.attach(sub, &req.method, req.params)))
                        } else {
                            shared.ctx.call(&req.method, req.params)
                        };
                        pending = Some((req.id, stop, later));
                    }
                    Err(e) => {
                        let e = ApiError::bad_request(format!("not a request: {e}"));
                        if !send(write, &line(&json!({ "id": null, "error": e })), sub).await? {
                            return Ok(());
                        }
                    }
                }
            }
            _ = sub.ready.notified() => {
                // A batch per pass, so a finished reply waits for at most one batch.
                for _ in 0..EVENT_BATCH {
                    match sub.pop() {
                        Err(_) => return Ok(()), // too far behind: dropped
                        Ok(None) => break,
                        Ok(Some(event)) => {
                            if !send(write, &event, sub).await? {
                                return Ok(());
                            }
                        }
                    }
                }
                if sub.has_lines() {
                    sub.ready.notify_one();
                }
            }
        }
    }
}

fn line(v: &Value) -> Vec<u8> {
    let mut out = serde_json::to_vec(v).expect("json");
    out.push(b'\n');
    out
}

/// Write one line. Ok(false) if `sub` is dropped meanwhile: that also frees a connection stuck
/// writing to a client that stopped reading.
async fn send(write: &mut OwnedWriteHalf, line: &[u8], sub: &Subscriber) -> io::Result<bool> {
    let mut out = std::pin::pin!(async {
        write.write_all(line).await?;
        write.flush().await
    });
    let mut woken = false;
    let written = loop {
        tokio::select! {
            r = &mut out => break r,
            _ = sub.ready.notified() => {
                if sub.is_dropped() {
                    return Ok(false);
                }
                woken = true;
            }
        }
    };
    if woken {
        sub.ready.notify_one(); // the main loop still has those events to write
    }
    written.map(|_| true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn bind_refuses_an_open_dir_and_leaves_it_alone() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(bind(&dir.path().join("server.sock")).await.is_err());
        let mode = std::fs::metadata(dir.path()).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o755, "LEON_RUNTIME_DIR=/tmp must not chmod /tmp");
    }

    #[tokio::test]
    async fn bind_makes_a_new_dir_private_and_the_socket_0600() {
        let parent = tempfile::tempdir().unwrap();
        let socket = parent.path().join("leon/server.sock");
        let _l = bind(&socket).await.unwrap();
        let mode = |p: &Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&parent.path().join("leon")), 0o700);
        assert_eq!(mode(&socket), 0o600);
    }
}
