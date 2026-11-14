//! The host terminal: raw mode and the alt screen while attached, given back on every way out
//! (detach, an error, a panic, a signal, and through the watchdog even SIGKILL).

use std::io::{BufWriter, Stdout, Write};
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::Mutex;

use ratatui::backend::CrosstermBackend;
use ratatui::crossterm::cursor::{Hide, Show};
use ratatui::crossterm::event::{DisableBracketedPaste, EnableBracketedPaste};
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen};
use ratatui::Terminal;

/// Whether the host is in raw mode on the alt screen (so `restore` has something to undo).
static ENTERED: AtomicBool = AtomicBool::new(false);
/// The watchdog's pipe: `e` says the host is now raw on the alt screen, `x` "restored, exit
/// quietly".
static WATCHDOG: AtomicI32 = AtomicI32::new(-1);
/// stderr as it was before `enter` pointed it at /dev/null.
static STDERR: AtomicI32 = AtomicI32::new(-1);
/// One restore at a time: a second caller waits until the host is back.
static RESTORING: Mutex<()> = Mutex::new(());

/// What `restore` writes, and the watchdog writes for a client that died without restoring.
const LEAVE: &[u8] = b"\x1b[?2004l\x1b[?1049l\x1b[?25h";

pub type Term = Terminal<CrosstermBackend<BufWriter<Stdout>>>;

pub struct Host {
    pub term: Term,
}

impl Host {
    /// Raw mode, the alt screen, bracketed paste, a panic hook that undoes them.
    pub fn enter() -> std::io::Result<Host> {
        enable_raw_mode()?;
        ENTERED.store(true, Ordering::SeqCst);
        // SAFETY: fd juggling on our own stderr and pipe
        unsafe {
            // leon-app's client reports some failures with eprintln!, which would scribble over
            // the screen; they go nowhere while attached, and stderr comes back on restore
            let null = libc::open(c"/dev/null".as_ptr(), libc::O_WRONLY | libc::O_CLOEXEC);
            let saved = libc::dup(2);
            if null >= 0 && saved >= 0 && libc::dup2(null, 2) >= 0 {
                libc::fcntl(saved, libc::F_SETFD, libc::FD_CLOEXEC);
                STDERR.store(saved, Ordering::SeqCst);
            }
            if null >= 0 {
                libc::close(null);
            }
            let fd = WATCHDOG.load(Ordering::SeqCst);
            if fd >= 0 {
                libc::write(fd, b"e".as_ptr().cast(), 1);
            }
        }
        // a frame goes out in one write, not a write per escape sequence
        let mut out = BufWriter::with_capacity(1 << 16, std::io::stdout());
        execute!(out, EnterAlternateScreen, EnableBracketedPaste, Hide)?;
        let prev = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            restore();
            prev(info);
            // a panic elsewhere would leave the main loop drawing on a cooked terminal
            if std::thread::current().name() != Some("main") {
                std::process::exit(101);
            }
        }));
        Ok(Host { term: Terminal::new(CrosstermBackend::new(out))? })
    }
}

impl Drop for Host {
    fn drop(&mut self) {
        let _ = self.term.backend_mut().flush();
        restore();
    }
}

/// Give the host back (alt screen left, cursor shown, cooked mode) and tell the watchdog.
/// Safe to call more than once: only the first undoes anything.
pub fn restore() {
    let _one = RESTORING.lock().unwrap_or_else(|e| e.into_inner());
    let saved = STDERR.swap(-1, Ordering::SeqCst);
    if saved >= 0 {
        // SAFETY: the dup `enter` made, put back once (the swap took it)
        unsafe {
            libc::dup2(saved, 2);
            libc::close(saved);
        }
    }
    if ENTERED.swap(false, Ordering::SeqCst) {
        let _ = execute!(std::io::stdout(), DisableBracketedPaste, LeaveAlternateScreen, Show);
        let _ = disable_raw_mode();
    }
    let fd = WATCHDOG.swap(-1, Ordering::SeqCst);
    if fd >= 0 {
        // SAFETY: our own pipe's write end, closed once (the swap took it)
        unsafe {
            libc::write(fd, b"x".as_ptr().cast(), 1);
            libc::close(fd);
        }
    }
}

/// Fork a watchdog that restores the host if this process dies without `restore` (SIGKILL). It
/// sits in a process group of its own, so a kill of ours doesn't reach it. Call it before any
/// thread starts: a fork copies only the calling thread.
pub fn watchdog() -> std::io::Result<()> {
    let err = std::io::Error::last_os_error;
    // SAFETY: plain libc calls; the child runs only async-signal-safe calls and `_exit`s.
    unsafe {
        let mut saved: libc::termios = std::mem::zeroed();
        if libc::tcgetattr(0, &mut saved) != 0 {
            return Err(err());
        }
        let mut fds = [0; 2];
        if libc::pipe(fds.as_mut_ptr()) != 0 {
            return Err(err());
        }
        // close-on-exec: nothing this process runs later may hold the pipe open (not atomic with
        // `pipe`, which is fine while no other thread can fork meanwhile)
        for fd in fds {
            if libc::fcntl(fd, libc::F_SETFD, libc::FD_CLOEXEC) != 0 {
                return Err(err());
            }
        }
        match libc::fork() {
            -1 => Err(err()),
            0 => {
                libc::close(fds[1]);
                libc::setpgid(0, 0);
                // a background group may still set the tty's modes with SIGTTOU ignored
                for s in [libc::SIGHUP, libc::SIGINT, libc::SIGQUIT, libc::SIGTERM, libc::SIGTSTP, libc::SIGTTOU] {
                    libc::signal(s, libc::SIG_IGN);
                }
                let (mut b, mut entered) = (0u8, false);
                loop {
                    let n = libc::read(fds[0], (&mut b as *mut u8).cast(), 1);
                    match (n, b) {
                        (1, b'x') => libc::_exit(0),
                        (1, _) => entered = true,
                        (-1, _) if err().raw_os_error() == Some(libc::EINTR) => {}
                        _ => break,
                    }
                }
                // the client died without restoring; the escapes only if it had entered
                if entered {
                    libc::write(1, LEAVE.as_ptr().cast(), LEAVE.len());
                }
                libc::tcsetattr(0, libc::TCSANOW, &saved);
                libc::_exit(0)
            }
            _ => {
                libc::close(fds[0]);
                WATCHDOG.store(fds[1], Ordering::SeqCst);
                Ok(())
            }
        }
    }
}

/// Block the signals that end a client (in this thread and every thread started after), so
/// `wait_signal` takes them instead of a handler. Call after starting any server: a child
/// inherits the mask.
pub fn block_signals() -> libc::sigset_t {
    // SAFETY: an empty set filled and applied to this thread
    unsafe {
        let mut set: libc::sigset_t = std::mem::zeroed();
        libc::sigemptyset(&mut set);
        for s in [libc::SIGHUP, libc::SIGINT, libc::SIGQUIT, libc::SIGTERM] {
            libc::sigaddset(&mut set, s);
        }
        libc::pthread_sigmask(libc::SIG_BLOCK, &set, std::ptr::null_mut());
        set
    }
}

/// The next of the blocked signals.
pub fn wait_signal(set: &libc::sigset_t) -> i32 {
    let mut sig = 0;
    // SAFETY: waits on a set `block_signals` made
    unsafe { libc::sigwait(set, &mut sig) };
    sig
}
