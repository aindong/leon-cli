//! A `Link` makes a host look like a local server: a Unix socket in a fresh private dir under the
//! runtime dir, served by threads in this process. Each connection to it runs ssh to the host's
//! `leon server proxy` and copies bytes both ways, so any client that takes a socket path (the
//! CLI, scripts, the app's `Remote`) works unchanged. Nothing listens on TCP. The link holds a
//! flock on `lock` in its dir while it lives, which is how another link tells a dead one's dir.

use std::io::{self, Read, Write};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::os::unix::io::AsRawFd;
use std::os::unix::net::{UnixListener, UnixStream};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use crate::Endpoint;

pub struct Link {
    socket: PathBuf,
    shared: Arc<Shared>,
    /// Flocked while the link lives (`remove_stale`).
    _lock: std::fs::File,
    /// Removed when the link drops.
    _dir: tempfile::TempDir,
}

struct Shared {
    name: String,
    endpoint: Endpoint,
    /// Whether a connection has reached the server. Later ones don't autostart: a server that
    /// went away was stopped (or its machine restarted), and a reconnect shouldn't undo that.
    reached: AtomicBool,
    /// `never_prompt` was called.
    batch: AtomicBool,
    /// What ssh last said on stderr, or why its run failed: the reason a host is unreachable.
    last_error: Mutex<Option<String>>,
    /// The link was dropped: the accept loop ends (and closes the socket) at its next connection.
    stop: AtomicBool,
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
        // locked under another name first: `remove_stale` elsewhere only tries `lock`
        let lock = std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(dir.path().join("lock.new"))?;
        // SAFETY: flock on a file we own; the lock ends with the file.
        if unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
            return Err(io::Error::last_os_error());
        }
        std::fs::rename(dir.path().join("lock.new"), dir.path().join("lock"))?;
        let socket = dir.path().join("s");
        let listener = UnixListener::bind(&socket)?;
        std::fs::set_permissions(&socket, std::fs::Permissions::from_mode(0o600))?;
        let shared = Arc::new(Shared {
            name: name.to_string(),
            endpoint: endpoint.clone(),
            reached: AtomicBool::new(!autostart),
            batch: AtomicBool::new(false),
            last_error: Mutex::new(None),
            stop: AtomicBool::new(false),
        });
        let serving = shared.clone();
        std::thread::Builder::new().name("leon-link".into()).spawn(move || {
            block_signals(); // and every thread it starts: `leon attach` takes them with sigwait
            for conn in listener.incoming() {
                if serving.stop.load(Ordering::SeqCst) {
                    return; // the listener closes with this thread
                }
                let Ok(conn) = conn else {
                    std::thread::sleep(std::time::Duration::from_millis(50)); // e.g. out of fds: don't spin
                    continue;
                };
                let shared = serving.clone();
                let _ = std::thread::Builder::new().name("leon-link-conn".into()).spawn(move || serve(conn, &shared));
            }
        })?;
        Ok(Link { socket, shared, _lock: lock, _dir: dir })
    }

    pub fn socket(&self) -> &Path {
        &self.socket
    }

    /// From now on ssh may not ask for a password: something else owns the terminal (`leon
    /// attach` in raw mode), and a prompt there would split the keystrokes between the two.
    pub fn never_prompt(&self) {
        self.shared.batch.store(true, Ordering::SeqCst);
    }

    /// The last line ssh wrote to stderr (`ssh: connect to host … Connection refused`), or why a
    /// run failed (leon missing on the host, ssh missing here); None while nothing went wrong.
    pub fn last_error(&self) -> Option<String> {
        self.shared.last_error.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }
}

impl Drop for Link {
    /// End the accept loop: it waits in `accept`, so wake it with a connection of our own, which it
    /// closes unserved. (The app opens a link per connection attempt, for as long as it runs.)
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::SeqCst);
        let _ = UnixStream::connect(&self.socket);
    }
}

impl Shared {
    fn failed(&self, why: String) {
        *self.last_error.lock().unwrap_or_else(|e| e.into_inner()) = Some(why);
    }

    /// Keep a line of ssh's as the reason: printable only (the app shows it), 200 characters at
    /// most, blank lines skipped.
    fn failed_with(&self, line: &str) {
        let shown = crate::shown_text(line, 200);
        if !shown.is_empty() {
            self.failed(shown);
        }
    }
}

/// One connection: ssh's stdin is what the client writes, ssh's stdout goes back to the client.
fn serve(conn: UnixStream, s: &Arc<Shared>) {
    // as the server does: the dir is private, and a connection that isn't ours gets no ssh
    // SAFETY: geteuid has no preconditions.
    if peer_uid(&conn) != Some(unsafe { libc::geteuid() }) {
        return;
    }
    let autostart = !s.reached.load(Ordering::SeqCst);
    let mut cmd = s.endpoint.ssh(autostart, s.batch.load(Ordering::SeqCst));
    // ssh gets its stdio and no other descriptor of ours: an ssh forked while another
    // connection's thread was making its pipes kept them for as long as it ran, and that
    // connection's `spawn` never came back (leon_core::spawn).
    leon_core::spawn::stdio_only(&mut cmd);
    // SAFETY: between fork and exec only sigemptyset and sigprocmask, both async-signal-safe.
    unsafe {
        cmd.pre_exec(|| {
            let mut none: libc::sigset_t = std::mem::zeroed();
            libc::sigemptyset(&mut none);
            match libc::sigprocmask(libc::SIG_SETMASK, &none, std::ptr::null_mut()) {
                0 => Ok(()),
                _ => Err(io::Error::last_os_error()),
            }
        });
    }
    let mut child = match cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn() {
        Ok(c) => c,
        Err(e) => {
            let why = format!("can't run {:?}: {e}", cmd.get_program());
            eprintln!("leon: {why}");
            s.failed(why);
            return;
        }
    };
    // ssh's messages still reach our stderr; the last one is kept as the reason
    let (read_all, errors) = std::sync::mpsc::channel::<()>();
    // once ssh got through, its stderr (a key exchange warning, say) is no reason for anything
    let answered = Arc::new(AtomicBool::new(false));
    if let Some(err) = child.stderr.take() {
        let (s, answered) = (s.clone(), answered.clone());
        let _ = std::thread::Builder::new().name("leon-link-err".into()).spawn(move || {
            let _read_all = read_all; // dropped at the end of ssh's stderr
                                      // the host writes what it likes here: a line is read 4 KiB at most at a time
            let mut err = io::BufReader::new(err);
            let mut line = Vec::new();
            while io::BufRead::read_until(&mut (&mut err).take(4096), b'\n', &mut line).is_ok_and(|n| n > 0) {
                let text = String::from_utf8_lossy(&line);
                eprintln!("{}", text.trim_end_matches('\n'));
                if !answered.load(Ordering::SeqCst) {
                    s.failed_with(&text);
                }
                line.clear();
            }
        });
    }
    let (Some(mut stdin), Some(mut stdout), Ok(mut from)) = (child.stdin.take(), child.stdout.take(), conn.try_clone())
    else {
        let _ = child.kill();
        let _ = child.wait();
        return;
    };
    // The client's end closing closes ssh's stdin, so the proxy on the host ends and ssh exits.
    let _ = std::thread::Builder::new().name("leon-link-up".into()).spawn(move || {
        crate::pump(&mut from, &mut stdin);
    });
    let mut to = &conn;
    let mut buf = vec![0; 64 << 10];
    while let Ok(n @ 1..) = stdout.read(&mut buf) {
        s.reached.store(true, Ordering::SeqCst);
        if !answered.swap(true, Ordering::SeqCst) {
            *s.last_error.lock().unwrap_or_else(|e| e.into_inner()) = None; // warnings before it worked
        }
        if to.write_all(&buf[..n]).is_err() {
            break;
        }
    }
    drop(stdout);
    // ssh's last words are in before anyone asks why it ended (bounded: a program ssh started
    // may still hold its stderr)
    let _ = errors.recv_timeout(std::time::Duration::from_secs(1));
    if child.wait().is_ok_and(|st| st.code() == Some(127)) {
        let name = &s.name;
        let why = format!(
            "leon isn't on {name}'s PATH or in ~/.local/bin: install leon on {name} (brew install aindong/leon/leon, \
             or a release tarball: https://github.com/aindong/leon-cli#from-a-release)"
        );
        eprintln!("leon: {why}");
        s.failed(why);
    }
    let _ = conn.shutdown(std::net::Shutdown::Both);
}

/// Remove the dirs of links whose client died without cleaning up (Ctrl-C, SIGKILL): nothing
/// holds their lock. Never connect to find out: a live link answers a connection by running ssh.
/// A dir without a lock file yet may be a link opening now.
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
        let opened = std::fs::OpenOptions::new().write(true).custom_flags(libc::O_NOFOLLOW).open(path.join("lock"));
        let Ok(lock) = opened else { continue };
        // SAFETY: flock on a file we just opened; it is released when `lock` drops.
        if unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0 {
            let _ = std::fs::remove_dir_all(&path);
        }
    }
}

/// The uid of the process at the other end (std's `peer_cred` isn't stable).
fn peer_uid(conn: &UnixStream) -> Option<libc::uid_t> {
    let fd = conn.as_raw_fd();
    #[cfg(target_os = "linux")]
    {
        // SAFETY: getsockopt writes at most `len` bytes into `cred`.
        let mut cred: libc::ucred = unsafe { std::mem::zeroed() };
        let mut len = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
        let ok = unsafe {
            libc::getsockopt(fd, libc::SOL_SOCKET, libc::SO_PEERCRED, (&mut cred as *mut libc::ucred).cast(), &mut len)
        } == 0;
        ok.then_some(cred.uid)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let (mut uid, mut gid) = (0, 0);
        // SAFETY: getpeereid writes the two ids.
        (unsafe { libc::getpeereid(fd, &mut uid, &mut gid) } == 0).then_some(uid)
    }
}

/// Block the signals that end a client in this thread (and the threads it starts), so they go
/// to the main thread: under `leon attach`, to its `sigwait`.
fn block_signals() {
    // SAFETY: an empty set filled and applied to this thread. ssh mustn't inherit it: on Linux
    // Rust's Command keeps the spawning thread's mask, so `serve` empties it in the child.
    unsafe {
        let mut set: libc::sigset_t = std::mem::zeroed();
        libc::sigemptyset(&mut set);
        for s in [libc::SIGHUP, libc::SIGINT, libc::SIGQUIT, libc::SIGTERM] {
            libc::sigaddset(&mut set, s);
        }
        libc::pthread_sigmask(libc::SIG_BLOCK, &set, std::ptr::null_mut());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_hosts_words_are_kept_printable_and_short() {
        let s = Shared {
            name: "box".into(),
            endpoint: "ssh://box".parse().unwrap(),
            reached: AtomicBool::new(false),
            batch: AtomicBool::new(false),
            last_error: Mutex::new(None),
            stop: AtomicBool::new(false),
        };
        s.failed_with("ssh: connect to host box port 22: Connection refused\r\n");
        assert_eq!(
            s.last_error.lock().unwrap().as_deref(),
            Some("ssh: connect to host box port 22: Connection refused")
        );
        s.failed_with("  \n");
        assert!(s.last_error.lock().unwrap().is_some(), "a blank line keeps the reason");
        s.failed_with(&format!("\x1b]0;title\x07{}\u{202e}", "x".repeat(1000)));
        let kept = s.last_error.lock().unwrap().clone().unwrap();
        assert!(kept.chars().count() <= 200 && !kept.chars().any(char::is_control), "{kept:?}");
        assert!(!kept.contains('\u{202e}'), "no bidi override: {kept:?}");
    }
}
