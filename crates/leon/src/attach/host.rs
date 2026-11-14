//! The host terminal: raw mode and the alt screen while attached, given back on every way out
//! (detach, an error, a panic, a signal, and through the watchdog even SIGKILL).

use std::io::{BufWriter, Stdout, Write};
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};

use ratatui::backend::CrosstermBackend;
use ratatui::crossterm::cursor::{Hide, Show};
use ratatui::crossterm::event::{DisableBracketedPaste, EnableBracketedPaste};
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen};
use ratatui::Terminal;

/// Whether the host is in raw mode on the alt screen (so `restore` has something to undo).
static ENTERED: AtomicBool = AtomicBool::new(false);
/// The watchdog's pipe: a byte on it says "restored, exit quietly".
static WATCHDOG: AtomicI32 = AtomicI32::new(-1);

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
        // a frame goes out in one write, not a write per escape sequence
        let mut out = BufWriter::with_capacity(1 << 16, std::io::stdout());
        execute!(out, EnterAlternateScreen, EnableBracketedPaste, Hide)?;
        let prev = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            restore();
            prev(info);
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
        // close-on-exec: a server this process starts later must not hold the pipe open
        for fd in fds {
            libc::fcntl(fd, libc::F_SETFD, libc::FD_CLOEXEC);
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
                let mut b = 0u8;
                let n = loop {
                    let n = libc::read(fds[0], (&mut b as *mut u8).cast(), 1);
                    if n >= 0 || err().raw_os_error() != Some(libc::EINTR) {
                        break n;
                    }
                };
                if n != 1 {
                    // the client died without restoring
                    libc::write(1, LEAVE.as_ptr().cast(), LEAVE.len());
                    libc::tcsetattr(0, libc::TCSANOW, &saved);
                }
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
