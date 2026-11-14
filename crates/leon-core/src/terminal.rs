//! A terminal: a program on a PTY, its output fed through alacritty's emulator on a reader
//! thread so the screen and scrollback can be read back at any time.

use std::fs::File;
use std::io::{self, Read, Write};
use std::os::fd::FromRawFd;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use alacritty_terminal::event::{Event, EventListener};
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Point};
use alacritty_terminal::term::{Config, Term, TermMode};
use alacritty_terminal::vte::ansi::Processor;
use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};

/// The terminal block namespace (`<ns>.terminal` in docs/spec.md).
pub const NAMESPACE: &str = "dev.leon.terminal";
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

/// Input for the program, written to the PTY by a thread of its own: a program that stops reading
/// fills the PTY's queue, and a write then blocks until it reads again.
type Writer = mpsc::Sender<Vec<u8>>;

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// Sends the emulator's answers to terminal queries (cursor position, device attributes) back to the program.
pub struct Replies(Writer);

impl EventListener for Replies {
    fn send_event(&self, event: Event) {
        if let Event::PtyWrite(text) = event {
            let _ = self.0.send(text.into_bytes());
        }
    }
}

struct Size {
    rows: u16,
    cols: u16,
}

impl Dimensions for Size {
    fn total_lines(&self) -> usize {
        self.rows.into()
    }
    fn screen_lines(&self) -> usize {
        self.rows.into()
    }
    fn columns(&self) -> usize {
        self.cols.into()
    }
}

pub struct Terminal {
    term: Arc<Mutex<Term<Replies>>>,
    writer: Writer,
    master: Mutex<Box<dyn MasterPty + Send>>,
    pid: i32,
    /// Set (under its lock) once the program has exited and before it is reaped.
    exited: Arc<Mutex<bool>>,
    /// Bumped whenever the screen may have changed, so a renderer can skip unchanged frames.
    generation: Arc<AtomicU64>,
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
        // SAFETY: returns a new close-on-exec fd that we own and hand to File.
        let dup = unsafe { libc::fcntl(fd, libc::F_DUPFD_CLOEXEC, 0) };
        if dup < 0 {
            return Err(io::Error::last_os_error());
        }
        let mut pty_in = unsafe { File::from_raw_fd(dup) };
        let (writer, input) = mpsc::channel::<Vec<u8>>();
        // Ends once every sender is gone, or when the PTY closes.
        std::thread::spawn(move || input.iter().try_for_each(|data| pty_in.write_all(&data)));
        let config = Config { scrolling_history: SCROLLBACK, ..Config::default() };
        let term = Arc::new(Mutex::new(Term::new(config, &Size { rows: ROWS, cols: COLS }, Replies(writer.clone()))));

        let screen = term.clone();
        let generation = Arc::new(AtomicU64::new(0));
        let changes = generation.clone();
        std::thread::spawn(move || read_loop(reader, &screen, &changes));
        let exited = Arc::new(Mutex::new(false));
        let done = exited.clone();
        std::thread::spawn(move || {
            wait_exit(pid);
            let status = {
                let mut done = lock(&done);
                // The zombie still holds the group ID, so this reaches only what the program left behind.
                // SAFETY: only sends a signal.
                unsafe { libc::kill(-pid, libc::SIGHUP) };
                *done = true; // under the lock, so Drop never signals a group ID that reaping freed
                reap(pid)
            };
            drop(child); // reaped above; dropping std's Child neither waits nor kills
            on_exit(status);
        });
        Ok(Terminal { term, writer, master: Mutex::new(pair.master), pid, exited, generation, command })
    }

    pub fn pid(&self) -> i32 {
        self.pid
    }

    /// Queue input for the program; never blocks.
    pub fn write(&self, data: &[u8]) -> io::Result<()> {
        self.writer
            .send(data.to_vec())
            .map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "the terminal has closed"))
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

    /// (rows, cols).
    pub fn size(&self) -> (u16, u16) {
        let term = lock(&self.term);
        (term.screen_lines() as u16, term.columns() as u16)
    }

    /// Resize the grid and the PTY (which signals SIGWINCH to the program). Both are at least 1.
    pub fn resize(&self, rows: u16, cols: u16) -> io::Result<()> {
        let (rows, cols) = (rows.max(1), cols.max(1));
        let size = PtySize { rows, cols, pixel_width: 0, pixel_height: 0 };
        lock(&self.master).resize(size).map_err(pty)?;
        lock(&self.term).resize(Size { rows, cols });
        self.generation.fetch_add(1, Ordering::Release);
        Ok(())
    }

    /// Changes so far; a renderer redraws when this moves.
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    /// Read the emulator's state (grid, cursor, modes) under its lock. Keep `f` short: the
    /// reader thread waits for it.
    pub fn with_term<R>(&self, f: impl FnOnce(&Term<Replies>) -> R) -> R {
        f(&lock(&self.term))
    }
}

impl Terminal {
    /// Hang up a still-running program: its process group (portable-pty makes the child a session
    /// leader) and the terminal's foreground job (e.g. vim started from a shell). An exited
    /// program's group was hung up when it exited, and its ID may belong to someone else by now,
    /// so it gets nothing.
    pub fn hang_up(&self) {
        let fd = lock(&self.master).as_raw_fd();
        let exited = lock(&self.exited);
        if *exited || self.pid <= 0 {
            return;
        }
        // SAFETY: only signals; the held lock keeps the leader unreaped, so both IDs are still ours.
        unsafe {
            let fg = fd.map_or(-1, |fd| libc::tcgetpgrp(fd));
            if fg > 0 && fg != self.pid {
                libc::kill(-fg, libc::SIGHUP);
            }
            libc::kill(-self.pid, libc::SIGHUP);
        }
    }

    /// SIGKILL the program's process group if the program still runs.
    pub fn kill(&self) {
        kill_group(self.pid, &self.exited);
    }

    pub fn has_exited(&self) -> bool {
        *lock(&self.exited)
    }
}

fn kill_group(pid: i32, exited: &Mutex<bool>) {
    let exited = lock(exited);
    if !*exited && pid > 0 {
        // SAFETY: only signals; the held lock keeps the group ID ours.
        unsafe { libc::kill(-pid, libc::SIGKILL) };
    }
}

impl Drop for Terminal {
    /// Hang up the program, and kill it if it still runs after a grace period.
    fn drop(&mut self) {
        if self.has_exited() {
            return;
        }
        self.hang_up();
        let (pid, exited) = (self.pid, self.exited.clone());
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_secs(2));
            kill_group(pid, &exited);
        });
    }
}

fn read_loop(mut reader: Box<dyn Read + Send>, term: &Mutex<Term<Replies>>, changes: &AtomicU64) {
    let mut parser: Processor = Processor::new();
    let mut buf = [0u8; 8192];
    loop {
        match reader.read(&mut buf) {
            // macOS reports a closed PTY as EIO rather than EOF.
            Ok(0) | Err(_) => return,
            Ok(n) => {
                parser.advance(&mut *lock(term), &buf[..n]);
                changes.fetch_add(1, Ordering::Release);
            }
        }
    }
}

/// Block until `pid` exits, leaving it unreaped (a zombie keeps its process group ID taken).
fn wait_exit(pid: i32) {
    loop {
        // SAFETY: waitid on our own child, writing into a local; WNOWAIT leaves it waitable.
        let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
        let r = unsafe { libc::waitid(libc::P_PID, pid as libc::id_t, &mut info, libc::WEXITED | libc::WNOWAIT) };
        if r == 0 || io::Error::last_os_error().kind() != io::ErrorKind::Interrupted {
            return;
        }
    }
}

/// Reap `pid` and turn its wait status into a shell-style exit status.
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
    fn writes_never_block_on_a_program_that_does_not_read() {
        // Raw mode: the line discipline queues input instead of dropping what doesn't fit.
        let (t, _rx) = spawn(&["sh", "-c", "stty raw -echo; echo ready; sleep 300"]);
        screen_until(&t, |s| s.contains("ready"));
        let t = Arc::new(t);
        let (done, finished) = mpsc::channel();
        let writer = t.clone();
        std::thread::spawn(move || {
            writer.write(&vec![b'x'; 256 << 10]).unwrap(); // far past the PTY's input buffer
            done.send(()).unwrap();
        });
        finished.recv_timeout(Duration::from_secs(5)).expect("write blocked on a full PTY");
    }

    #[test]
    fn dropping_hangs_up_the_program() {
        let (t, rx) = spawn(&["sleep", "300"]);
        drop(t);
        assert_eq!(rx.recv_timeout(Duration::from_secs(10)).unwrap(), 128 + libc::SIGHUP);
    }
}
