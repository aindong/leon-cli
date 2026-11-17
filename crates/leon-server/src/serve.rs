//! The socket server: NDJSON requests in, NDJSON responses out, and pushed event lines on
//! connections that attached (docs/architecture.md).
//!
//! Access control is the private runtime dir (leon_core::paths::ensure_private_dir) plus a peer
//! uid check on every connection: once terminals exist, a connection can run code as the user.

use std::io;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use leon_core::ApiError;
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::signal::unix::{signal, SignalKind};
use tokio::sync::mpsc;

use crate::events::{is_attach, Subscriber};
use crate::persist::{self, Persist};
use crate::screens::Screens;
use crate::{Ctx, Later};

/// Event lines written between two looks at replies and requests.
const EVENT_BATCH: usize = 64;

/// The longest request line accepted (1 MiB); a longer one gets an error and the connection closes.
pub const MAX_LINE: usize = 1 << 20;

pub struct Options {
    pub socket: PathBuf,
    /// An autostarted server (`Ctx::is_autostarted`) exits after this long with no sessions and no connections.
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

pub(crate) struct Shared {
    pub(crate) ctx: Arc<Ctx>,
    pub(crate) open: AtomicUsize,
    last_activity: Mutex<Instant>,
}

impl Shared {
    pub(crate) fn touch(&self) {
        *self.last_activity.lock().unwrap_or_else(|e| e.into_inner()) = Instant::now();
    }
}

/// Counts an open connection for the idle check; the drop also covers a panicking handler.
pub(crate) struct Open(pub(crate) Arc<Shared>);

impl Drop for Open {
    fn drop(&mut self) {
        self.0.touch(); // before the decrement, so the idle clock never sees 0 open with a stale time
        self.0.open.fetch_sub(1, Ordering::SeqCst);
    }
}

/// A bound socket, ready for `serve_on`.
pub struct Listener {
    listener: UnixListener,
    /// The socket's (dev, inode), so shutdown only removes our own socket.
    ours: (u64, u64),
    /// Where to save on stop, and the lock that keeps the state dir ours (`Listener::persist`).
    saving: Option<(PathBuf, std::fs::File)>,
    /// `server.lock`, held while the server lives (`bind`); none if a server that lost its path holds it.
    alive: Option<std::fs::File>,
}

impl Listener {
    /// Restore the saved sessions into `ctx` (unless `p.restore` is false) and save them when
    /// `serve_on` stops. Call before anything uses `ctx`: an app that opens a first session when
    /// there is none must see the restored ones.
    pub fn persist(&mut self, ctx: &Arc<Ctx>, p: &Persist) {
        self.saving = persist::start(ctx, p);
        *crate::events::lock(&ctx.state_file) = self.saving.as_ref().map(|(f, _)| f.clone());
    }
}

/// Bind the socket (refusing if a live server already owns it), then serve `ctx` until stopped.
pub async fn serve(ctx: Arc<Ctx>, opts: Options) -> io::Result<()> {
    serve_on(ctx, listen(&opts.socket).await?, opts).await
}

/// Bind the socket in its private runtime dir, refusing if a live server already owns it.
pub async fn listen(socket: &Path) -> io::Result<Listener> {
    raise_fd_limit();
    let (listener, ours, alive) = bind(socket).await?;
    Ok(Listener { listener, ours, saving: None, alive })
}

/// Each terminal holds three descriptors and each client one, and launchd starts the app (so its
/// embedded server too) with a soft limit of 256: raise it to the hard limit, at most 10240
/// (macOS's OPEN_MAX, which its setrlimit won't go past; programs inherit it).
fn raise_fd_limit() {
    const MOST: libc::rlim_t = 10240;
    let mut r = libc::rlimit { rlim_cur: 0, rlim_max: 0 };
    // SAFETY: getrlimit and setrlimit on our own process, with a valid struct.
    unsafe {
        if libc::getrlimit(libc::RLIMIT_NOFILE, &mut r) == 0 && r.rlim_cur < r.rlim_max.min(MOST) {
            r.rlim_cur = r.rlim_max.min(MOST);
            libc::setrlimit(libc::RLIMIT_NOFILE, &r);
        }
    }
}

/// Serve `ctx` on a bound socket until stopped (`server.stop`, the embedding app quitting,
/// SIGTERM, SIGINT, SIGHUP, or idle when autostarted). With `Listener::persist`, the sessions are
/// saved before every program ends.
pub async fn serve_on(ctx: Arc<Ctx>, bound: Listener, opts: Options) -> io::Result<()> {
    let Listener { listener, ours, saving, alive } = bound;
    let (state_file, state_lock) = saving.unzip();
    let mut term = signal(SignalKind::terminate())?;
    let mut int = signal(SignalKind::interrupt())?;
    let mut hup = signal(SignalKind::hangup())?; // `server run`'s terminal closed
    let shared = Arc::new(Shared { ctx, open: AtomicUsize::new(0), last_activity: Mutex::new(Instant::now()) });
    let mut tick = tokio::time::interval(Duration::from_millis(200));
    let mut autosave = persist::Autosave::default();
    let tailnet = tokio::spawn(crate::tailnet::supervise(shared.clone())); // serve_tailscale
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
                    let (read, write) = stream.into_split();
                    let _ = connection(read, write, &guard.0).await;
                    drop(guard);
                });
            }
            _ = shared.ctx.shutdown.notified() => break,
            _ = term.recv() => break,
            _ = int.recv() => break,
            _ = hup.recv() => break,
            _ = tick.tick() => {
                let idle = shared.ctx.is_autostarted()
                    && shared.open.load(Ordering::SeqCst) == 0
                    && shared.ctx.state().sessions.is_empty()
                    && shared.last_activity.lock().unwrap_or_else(|e| e.into_inner()).elapsed() >= opts.idle_exit;
                if idle { break; }
                shared.ctx.sweep_parked();
                shared.ctx.check_foregrounds(); // a program that starts silently prints nothing to notice
                if let Some(file) = &state_file {
                    autosave.tick(&shared.ctx, file);
                }
            }
        }
    }
    tailnet.abort(); // no tailnet listener or connection outlives the loop
    let _ = tailnet.await;
    if let Some(file) = &state_file {
        let st = shared.ctx.state();
        // Under the lock, as in end_programs: no session is created or changed after the save.
        shared.ctx.stopping.store(true, Ordering::SeqCst);
        let done = match shared.ctx.discard.load(Ordering::SeqCst) {
            true => persist::discard(file),
            false => persist::save(&st, file),
        };
        drop(st);
        if let Err(e) = done {
            eprintln!("leon: warning: can't save sessions to {}: {e}", file.display());
        }
    }
    shared.ctx.end_programs(Duration::from_secs(2)).await;
    // Before the socket goes, so a server started next finds the state dir free.
    drop(state_lock);
    // Only remove the socket if the path is still ours; a newer server may have bound it.
    if std::fs::symlink_metadata(&opts.socket).is_ok_and(|m| (m.dev(), m.ino()) == ours) {
        let _ = std::fs::remove_file(&opts.socket);
    }
    drop(alive);
    Ok(())
}

fn same_user(stream: &UnixStream) -> bool {
    // SAFETY: geteuid has no preconditions.
    stream.peer_cred().is_ok_and(|c| c.uid() == unsafe { libc::geteuid() })
}

/// Bind in a private runtime dir. Returns the listener, the socket's (dev, inode) and the lock
/// beside the socket, which the server holds for its life: a held lock means a live server, so no
/// one unlinks its socket. Asking the socket can't tell: a busy server's full backlog refuses
/// connects on macOS, just as a stale socket does.
async fn bind(socket: &Path) -> io::Result<(UnixListener, (u64, u64), Option<std::fs::File>)> {
    let dir = socket.parent().unwrap_or(Path::new("."));
    leon_core::paths::ensure_private_dir(dir)?;
    // A server holds server.lock for its life; one holding it now is live, binding or stopping. Wait for the lock or
    // for its socket to answer, so the client that started this one finds a server either way.
    let end = Instant::now() + Duration::from_secs(2);
    let alive = loop {
        match lock(&dir.join("server.lock")) {
            Ok(f) => break Some(f),
            Err(e) if e.raw_os_error() == Some(libc::EWOULDBLOCK) => {}
            Err(e) => return Err(e),
        }
        if UnixStream::connect(socket).await.is_ok() {
            return Err(in_use(socket));
        }
        if Instant::now() >= end {
            // Never answered: busy, so leave its socket be; or it lost its path, so take the path.
            if std::fs::symlink_metadata(socket).is_ok() {
                return Err(io::Error::new(
                    io::ErrorKind::AddrInUse,
                    format!("{} is held by a leon server that isn't answering", socket.display()),
                ));
            }
            break None;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    };
    // Still ask the socket: a server from before the lock was held for life doesn't hold it.
    if std::fs::symlink_metadata(socket).is_ok() {
        match UnixStream::connect(socket).await {
            Ok(_) => return Err(in_use(socket)),
            // Only a refused connection means stale; anything else may be a live server we can't reach.
            Err(e) if e.kind() == io::ErrorKind::ConnectionRefused => std::fs::remove_file(socket)?,
            Err(e) => return Err(io::Error::new(e.kind(), format!("{} is in use: {e}", socket.display()))),
        }
    }
    leon_core::paths::check_socket_path(socket)?;
    let listener = UnixListener::bind(socket)?;
    std::fs::set_permissions(socket, std::fs::Permissions::from_mode(0o600))?;
    let m = std::fs::symlink_metadata(socket)?;
    Ok((listener, (m.dev(), m.ino()), alive))
}

fn in_use(socket: &Path) -> io::Error {
    io::Error::new(io::ErrorKind::AddrInUse, format!("a leon server already listens on {}", socket.display()))
}

/// An exclusive flock on `path`, or EWOULDBLOCK if another holds it; released when the file drops.
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
    if unsafe { libc::flock(f.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(f)
}

/// A connection's reader task, aborted when the connection ends, also when it is cut mid-way
/// (tailnet.rs, when serving stops): the read half closes with it.
struct Reader(tokio::task::JoinHandle<io::Result<()>>);

impl Drop for Reader {
    fn drop(&mut self) {
        self.0.abort();
    }
}

/// One request line from the client, or the end of a line too long to read.
enum Line {
    Text(Vec<u8>),
    TooLarge,
}

/// One connection (the Unix socket's, or one tailnet.rs let in): requests are answered one at a
/// time, in order, while events the connection attached to are written between replies.
pub(crate) async fn connection<R, W>(read: R, mut write: W, shared: &Shared) -> io::Result<()>
where
    R: AsyncRead + Unpin + Send + 'static,
    W: AsyncWrite + Unpin,
{
    let (tx, mut lines) = mpsc::channel(1);
    let _reader = Reader(tokio::spawn(read_lines(read, tx)));
    let sub = Arc::new(Subscriber::default());
    let mut screens = Screens::default();
    let result = serve_lines(&mut lines, &mut write, &sub, &mut screens, shared).await;
    shared.ctx.park(&sub); // a resumable stream waits for its client to come back
    result
}

async fn read_lines(read: impl AsyncRead + Unpin, tx: mpsc::Sender<Line>) -> io::Result<()> {
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
    write: &mut (impl AsyncWrite + Unpin),
    sub: &Arc<Subscriber>,
    screens: &mut Screens,
    shared: &Shared,
) -> io::Result<()> {
    // Order: a finished call's reply, then the next request, then events, then screens, so an
    // event flood can't hold up replies and an attach reply precedes its events.
    // The call being answered: its id, whether it stops the server, and its result.
    let mut pending: Option<(Value, bool, Later<Value>)> = None;
    let screens_ready = screens.ready.clone();
    loop {
        let due = screens.due();
        tokio::select! {
            biased;
            result = async { pending.as_mut().expect("guarded").2.as_mut().await }, if pending.is_some() => {
                let (id, stop, _) = pending.take().expect("guarded");
                let stop = stop && result.is_ok();
                let reply = match result {
                    Ok(result) => json!({ "id": id, "result": result }),
                    Err(e) => json!({ "id": id, "error": e }),
                };
                let sent = send(write, &line(&reply), sub).await;
                if stop {
                    // After this connection's own stop reply is out, or failed to go: it was accepted.
                    shared.ctx.shutdown.notify_one();
                }
                if !sent? {
                    return Ok(());
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
                        let later: Later<Value> = if is_attach(&req.method) {
                            // Here, not in the registry: attaching subscribes this connection.
                            Box::pin(std::future::ready(shared.ctx.attach(sub, &req.method, req.params)))
                        } else if req.method == "block.watch" {
                            Box::pin(std::future::ready(screens.watch(&shared.ctx, req.params)))
                        } else {
                            shared.ctx.call(&req.method, req.params)
                        };
                        pending = Some((req.id, req.method == "server.stop", later));
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
            // a watched block changed, or one held back by the screens' gap is due
            _ = async {
                match due {
                    Some(at) => tokio::select! { _ = screens_ready.notified() => {}, _ = tokio::time::sleep_until(at) => {} },
                    None => screens_ready.notified().await,
                }
            } => {
                for line in screens.take() {
                    if !send(write, &line, sub).await? {
                        return Ok(());
                    }
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
async fn send(write: &mut (impl AsyncWrite + Unpin), line: &[u8], sub: &Subscriber) -> io::Result<bool> {
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
