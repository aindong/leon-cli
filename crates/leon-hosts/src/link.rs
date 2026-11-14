//! A `Link` makes a host look like a local server: a Unix socket in a fresh private dir under the
//! runtime dir, served by threads in this process. Each connection to it runs ssh to the host's
//! `leon server proxy` and copies bytes both ways, so any client that takes a socket path (the
//! CLI, scripts, the app's `Remote`) works unchanged. Nothing listens on TCP.

use std::io::{self, Read, Write};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::Endpoint;

pub struct Link {
    socket: PathBuf,
    /// Removed when the link drops.
    _dir: tempfile::TempDir,
}

struct Shared {
    name: String,
    endpoint: Endpoint,
    /// Whether a connection has reached the server. Later ones don't autostart: a server that
    /// went away was stopped (or its machine restarted), and a reconnect shouldn't undo that.
    reached: AtomicBool,
}

impl Link {
    /// Open a link to `endpoint`, called `name` in messages (a label, or the host). With
    /// `autostart`, the first connection starts a server on the host if none runs.
    pub fn open(name: &str, endpoint: &Endpoint, autostart: bool) -> io::Result<Link> {
        let runtime = leon_core::paths::runtime_dir();
        leon_core::paths::ensure_private_dir(&runtime)?;
        remove_stale(&runtime);
        let private = std::fs::Permissions::from_mode(0o700);
        let dir = tempfile::Builder::new().prefix("link-").permissions(private).tempdir_in(&runtime)?;
        let socket = dir.path().join("s");
        let listener = UnixListener::bind(&socket)?;
        std::fs::set_permissions(&socket, std::fs::Permissions::from_mode(0o600))?;
        let shared = Arc::new(Shared {
            name: name.to_string(),
            endpoint: endpoint.clone(),
            reached: AtomicBool::new(!autostart),
        });
        std::thread::Builder::new().name("leon-link".into()).spawn(move || {
            block_signals(); // and every thread it starts: `leon attach` takes them with sigwait
            for conn in listener.incoming().flatten() {
                let shared = shared.clone();
                let _ = std::thread::Builder::new().name("leon-link-conn".into()).spawn(move || serve(conn, &shared));
            }
        })?;
        Ok(Link { socket, _dir: dir })
    }

    pub fn socket(&self) -> &Path {
        &self.socket
    }
}

/// One connection: ssh's stdin is what the client writes, ssh's stdout goes back to the client.
fn serve(conn: UnixStream, s: &Shared) {
    let autostart = !s.reached.load(Ordering::SeqCst);
    let mut cmd = s.endpoint.ssh(autostart);
    let mut child = match cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).spawn() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("leon: can't run {:?}: {e}", cmd.get_program());
            return;
        }
    };
    let (Some(mut stdin), Some(mut stdout), Ok(mut from)) = (child.stdin.take(), child.stdout.take(), conn.try_clone())
    else {
        return;
    };
    // The client's end closing closes ssh's stdin, so the proxy on the host ends and ssh exits.
    let _ = std::thread::Builder::new().name("leon-link-up".into()).spawn(move || {
        let _ = io::copy(&mut from, &mut stdin);
    });
    let mut to = &conn;
    let mut buf = vec![0; 64 << 10];
    while let Ok(n @ 1..) = stdout.read(&mut buf) {
        s.reached.store(true, Ordering::SeqCst);
        if to.write_all(&buf[..n]).is_err() {
            break;
        }
    }
    drop(stdout);
    if child.wait().is_ok_and(|st| st.code() == Some(127)) {
        let name = &s.name;
        eprintln!("leon: leon isn't on {name}'s PATH or in ~/.local/bin: install leon on {name}");
    }
    let _ = conn.shutdown(std::net::Shutdown::Both);
}

/// Remove the dirs of links whose client died without cleaning up (Ctrl-C, SIGKILL): the socket
/// is there and nothing answers. A dir whose socket isn't bound yet may be a link opening now.
fn remove_stale(runtime: &Path) {
    let Ok(entries) = std::fs::read_dir(runtime) else { return };
    // SAFETY: geteuid has no preconditions.
    let me = unsafe { libc::geteuid() };
    for e in entries.flatten() {
        let path = e.path();
        let ours = std::fs::symlink_metadata(&path).is_ok_and(|m| m.is_dir() && m.uid() == me);
        if !ours || !e.file_name().to_string_lossy().starts_with("link-") {
            continue;
        }
        if UnixStream::connect(path.join("s")).is_err_and(|e| e.kind() == io::ErrorKind::ConnectionRefused) {
            let _ = std::fs::remove_dir_all(&path);
        }
    }
}

/// Block the signals that end a client in this thread (and the threads it starts), so they go
/// to the main thread: under `leon attach`, to its `sigwait`.
fn block_signals() {
    // SAFETY: an empty set filled and applied to this thread. Rust's Command resets the mask in
    // children, so ssh doesn't inherit it.
    unsafe {
        let mut set: libc::sigset_t = std::mem::zeroed();
        libc::sigemptyset(&mut set);
        for s in [libc::SIGHUP, libc::SIGINT, libc::SIGQUIT, libc::SIGTERM] {
            libc::sigaddset(&mut set, s);
        }
        libc::pthread_sigmask(libc::SIG_BLOCK, &set, std::ptr::null_mut());
    }
}
