//! A terminal: a program on a PTY, its output fed through alacritty's emulator on a reader
//! thread so the screen and scrollback can be read back at any time.

use std::fs::File;
use std::io::{self, Read, Write};
use std::os::fd::FromRawFd;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use alacritty_terminal::event::{Event, EventListener};
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Point};
use alacritty_terminal::term::{Config, Term, TermMode};
use alacritty_terminal::vte::ansi::Processor;
use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};

pub const ROWS: u16 = 24;
pub const COLS: u16 = 80;
/// shortcut: scrollback is counted in lines, not the spec's ~64 MiB; revisit with M8's benchmarks.
const SCROLLBACK: usize = 10_000;

/// What to run.
pub struct Spec {
    /// argv; empty runs the login shell (`$SHELL`, else the passwd entry, with a `-` argv0).
    pub command: Vec<String>,
    pub cwd: Option<PathBuf>,
    /// Set on top of the server's own environment.
    pub env: Vec<(String, String)>,
}

type Writer = Arc<Mutex<File>>;

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// Sends the emulator's answers to terminal queries (cursor position, device attributes) back to the program.
struct Replies(Writer);

impl EventListener for Replies {
    fn send_event(&self, event: Event) {
        if let Event::PtyWrite(text) = event {
            let _ = lock(&self.0).write_all(text.as_bytes());
        }
    }
}

struct Size;

impl Dimensions for Size {
    fn total_lines(&self) -> usize {
        ROWS.into()
    }
    fn screen_lines(&self) -> usize {
        ROWS.into()
    }
    fn columns(&self) -> usize {
        COLS.into()
    }
}

pub struct Terminal {
    term: Arc<Mutex<Term<Replies>>>,
    writer: Writer,
    _master: Mutex<Box<dyn MasterPty + Send>>,
    pid: i32,
    exited: Arc<AtomicBool>,
    /// The argv that runs (the login shell's path when `Spec::command` was empty).
    pub command: Vec<String>,
}

impl std::fmt::Debug for Terminal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Terminal").field("pid", &self.pid).field("command", &self.command).finish()
    }
}

impl Terminal {
    /// Start `spec` on a new PTY. `on_exit` runs on a waiter thread with the exit status
    /// (128 + signal for a killed program) once the child has been reaped.
    pub fn spawn(spec: Spec, on_exit: impl FnOnce(i32) + Send + 'static) -> io::Result<Terminal> {
        let size = PtySize { rows: ROWS, cols: COLS, pixel_width: 0, pixel_height: 0 };
        let pair = native_pty_system().openpty(size).map_err(pty)?;
        let mut cmd = if spec.command.is_empty() {
            CommandBuilder::new_default_prog()
        } else {
            CommandBuilder::from_argv(spec.command.iter().map(Into::into).collect())
        };
        if let Some(dir) = &spec.cwd {
            // portable-pty silently falls back to $HOME for a missing dir; refuse instead.
            if !dir.is_dir() {
                return Err(io::Error::new(io::ErrorKind::NotFound, format!("no directory {}", dir.display())));
            }
            cmd.cwd(dir);
        }
        for (k, v) in &spec.env {
            cmd.env(k, v);
        }
        let command = if spec.command.is_empty() { vec![cmd.get_shell()] } else { spec.command };
        let child = pair.slave.spawn_command(cmd).map_err(pty)?;
        drop(pair.slave); // the child holds the only slave fd, so its exit ends our reads
        let pid = child.process_id().map_or(0, |p| p as i32);

        let reader = pair.master.try_clone_reader().map_err(pty)?;
        // Write through our own dup of the master: portable-pty's writer types "\n" + EOF when dropped,
        // which would run whatever was half-typed in a shell as its block closes.
        let fd = pair.master.as_raw_fd().ok_or_else(|| io::Error::other("the PTY has no fd"))?;
        // SAFETY: dup returns a new fd that we own and hand to File.
        let dup = unsafe { libc::dup(fd) };
        if dup < 0 {
            return Err(io::Error::last_os_error());
        }
        let writer: Writer = Arc::new(Mutex::new(unsafe { File::from_raw_fd(dup) }));
        let config = Config { scrolling_history: SCROLLBACK, ..Config::default() };
        let term = Arc::new(Mutex::new(Term::new(config, &Size, Replies(writer.clone()))));

        let screen = term.clone();
        std::thread::spawn(move || read_loop(reader, &screen));
        let exited = Arc::new(AtomicBool::new(false));
        let done = exited.clone();
        std::thread::spawn(move || {
            let status = reap(pid);
            drop(child); // reaped above; dropping std's Child neither waits nor kills
            done.store(true, Ordering::SeqCst);
            on_exit(status);
        });
        Ok(Terminal { term, writer, _master: Mutex::new(pair.master), pid, exited, command })
    }

    pub fn pid(&self) -> i32 {
        self.pid
    }

    pub fn write(&self, data: &[u8]) -> io::Result<()> {
        lock(&self.writer).write_all(data)
    }

    /// Press one key given in binding syntax (`enter`, `ctrl+c`, `up`).
    pub fn key(&self, key: &str) -> Result<(), String> {
        let app_cursor = lock(&self.term).mode().contains(TermMode::APP_CURSOR);
        let bytes = crate::keys::encode(key, app_cursor)?;
        self.write(&bytes).map_err(|e| e.to_string())
    }

    /// Scrollback and screen as text, oldest line first. `trim` drops trailing spaces and blank lines.
    pub fn text(&self, trim: bool) -> String {
        let term = lock(&self.term);
        let start = Point::new(term.topmost_line(), Column(0));
        let end = Point::new(term.bottommost_line(), term.last_column());
        let text = term.bounds_to_string(start, end);
        if !trim {
            return text;
        }
        let lines: Vec<&str> = text.lines().map(str::trim_end).collect();
        let keep = lines.iter().rposition(|l| !l.is_empty()).map_or(0, |i| i + 1);
        lines[..keep].join("\n")
    }

    pub fn size(&self) -> (u16, u16) {
        (ROWS, COLS)
    }
}

impl Drop for Terminal {
    /// Hang up the program's whole process group (portable-pty makes the child a session leader),
    /// and kill it if it is still running after a grace period.
    fn drop(&mut self) {
        let pid = self.pid;
        if pid <= 0 {
            return;
        }
        // SAFETY: only sends signals. Once the group is empty its ID could be reused by another
        // group leader, a window we accept (tmux makes the same trade).
        unsafe { libc::kill(-pid, libc::SIGHUP) };
        let exited = self.exited.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_secs(2));
            if !exited.load(Ordering::SeqCst) {
                unsafe { libc::kill(-pid, libc::SIGKILL) };
            }
        });
    }
}

fn read_loop(mut reader: Box<dyn Read + Send>, term: &Mutex<Term<Replies>>) {
    let mut parser: Processor = Processor::new();
    let mut buf = [0u8; 8192];
    loop {
        match reader.read(&mut buf) {
            // macOS reports a closed PTY as EIO rather than EOF.
            Ok(0) | Err(_) => return,
            Ok(n) => parser.advance(&mut *lock(term), &buf[..n]),
        }
    }
}

/// Wait for `pid` and turn its wait status into a shell-style exit status.
fn reap(pid: i32) -> i32 {
    let mut status = 0;
    loop {
        // SAFETY: waitpid on our own child, writing into a local.
        let r = unsafe { libc::waitpid(pid, &mut status, 0) };
        if r == pid {
            break;
        }
        if r < 0 && io::Error::last_os_error().kind() != io::ErrorKind::Interrupted {
            return 1;
        }
    }
    if libc::WIFEXITED(status) {
        libc::WEXITSTATUS(status)
    } else if libc::WIFSIGNALED(status) {
        128 + libc::WTERMSIG(status)
    } else {
        1
    }
}

fn pty(e: impl std::fmt::Display) -> io::Error {
    io::Error::other(e.to_string())
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;

    use super::*;

    fn spawn(command: &[&str]) -> (Terminal, mpsc::Receiver<i32>) {
        let (tx, rx) = mpsc::channel();
        let spec = Spec { command: command.iter().map(|s| s.to_string()).collect(), cwd: None, env: vec![] };
        let t = Terminal::spawn(spec, move |code| tx.send(code).unwrap()).unwrap();
        (t, rx)
    }

    fn screen_until(t: &Terminal, pred: impl Fn(&str) -> bool) -> String {
        let end = std::time::Instant::now() + Duration::from_secs(10);
        loop {
            let text = t.text(true);
            if pred(&text) || std::time::Instant::now() > end {
                return text;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    #[test]
    fn runs_a_program_and_reads_its_screen() {
        let (t, rx) = spawn(&["sh", "-c", "read line; echo got-$line; exit 4"]);
        t.write(b"abc\r").unwrap();
        let text = screen_until(&t, |s| s.contains("got-abc"));
        assert!(text.lines().any(|l| l == "got-abc"), "{text:?}");
        assert_eq!(rx.recv_timeout(Duration::from_secs(10)).unwrap(), 4);
    }

    #[test]
    fn dropping_hangs_up_the_program() {
        let (t, rx) = spawn(&["sleep", "300"]);
        drop(t);
        assert_eq!(rx.recv_timeout(Duration::from_secs(10)).unwrap(), 128 + libc::SIGHUP);
    }
}
