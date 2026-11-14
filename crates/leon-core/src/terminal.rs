//! A terminal: a program on a PTY, its output fed through alacritty's emulator on a reader
//! thread so the screen and scrollback can be read back at any time. The reader also tracks OSC
//! 7501 program status and reports title, pwd, bell and status changes; the waiter reports the exit.

use std::fs::File;
use std::io::{self, Read, Write};
use std::os::fd::FromRawFd;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use alacritty_terminal::event::{Event, EventListener};
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Point};
use alacritty_terminal::term::{Config, Term, TermMode};
use alacritty_terminal::vte::ansi::Processor;
use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};
use tokio::sync::watch;

use crate::program_status::{coalesce, strip_hidden, Change, Record, Tracker};

/// The terminal block namespace (`<ns>.terminal` in docs/spec.md).
#[path = "terminal_cwd.rs"]
mod cwd;
use cwd::{process_cwd, pwd_path, pwd_url};
#[path = "terminal_reading.rs"]
mod reading;
use reading::{Reading, Reported};

pub const NAMESPACE: &str = "dev.leon.terminal";
pub const ROWS: u16 = 24;
pub const COLS: u16 = 80;
/// The largest grid (rows, cols): more than a 6K display holds at 8 pt.
pub const MAX_SIZE: (u16, u16) = (500, 1000);
/// shortcut: scrollback is counted in lines, not the spec's ~64 MiB; revisit with M8's benchmarks.
const SCROLLBACK: usize = 10_000;
/// Answers to terminal queries waiting to be written; past this a program that queries but never
/// reads gets no more, so the writer queue can't grow without bound.
const REPLY_BUDGET: usize = 64 << 10;
/// Titles and pwds in events are cut to this many bytes.
const MAX_TEXT: usize = 4096;
/// How long the exit report waits for the PTY's EOF (background jobs may hold the PTY open). What
/// was read by then is always parsed before the report.
const DRAIN_WAIT: Duration = Duration::from_secs(1);
/// Reads queued between the PTY and the emulator. The tty itself buffers only ~1 KiB, so a reader
/// that stops to parse stalls the program; this queue lets reading go on meanwhile. At most
/// `QUEUED_READS` x `READ_SIZE` bytes wait, then reading waits for the parser.
const QUEUED_READS: usize = 64;
const READ_SIZE: usize = 16 << 10;
/// Queued reads are parsed together up to this many bytes, one lock and one report per batch.
const BATCH: usize = 64 << 10;

/// What a terminal reports, from its reader and waiter threads. Per batch of PTY reads (what
/// arrived while the last batch was parsed, up to `BATCH` bytes) there is at most one title, one
/// pwd and one bell (the last title and pwd win, and only if they changed), reported in that order
/// rather than stream order.
#[derive(Debug)]
pub enum Notice {
    /// The program exited with this status (128 + signal), after its last output was read.
    Exited(i32),
    Title(String),
    /// OSC 7: the working directory as a `file://host/path` URL.
    Pwd(String),
    Bell,
    ProgramStatus(Change),
}

type Notify = Arc<dyn Fn(Notice) + Send + Sync>;

/// What to run.
pub struct Spec {
    /// argv; empty runs the login shell (`$SHELL`, else the passwd entry, with a `-` argv0).
    pub command: Vec<String>,
    pub cwd: Option<PathBuf>,
    /// Set on top of the server's own environment.
    pub env: Vec<(String, String)>,
}

/// Input for the program, written to the PTY by a thread of its own: a program that stops reading
/// fills the PTY's queue, and a write then blocks until it reads again. Each chunk says whether it
/// is a reply, so the thread can give the reply budget back.
#[derive(Clone)]
struct Writer {
    tx: mpsc::Sender<(Vec<u8>, bool)>,
    queued_replies: Arc<AtomicUsize>,
}

impl Writer {
    fn input(&self, data: Vec<u8>) -> io::Result<()> {
        self.tx.send((data, false)).map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "the terminal has closed"))
    }

    /// Queue an answer to a terminal query, unless the reply budget is spent.
    fn reply(&self, data: Vec<u8>) {
        let n = data.len();
        if self.queued_replies.fetch_add(n, Ordering::SeqCst) + n > REPLY_BUDGET || self.tx.send((data, true)).is_err()
        {
            self.queued_replies.fetch_sub(n, Ordering::SeqCst);
        }
    }
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// What the emulator saw during one batch, reported once the batch is parsed.
#[derive(Default)]
struct Seen {
    title: Option<String>,
    bell: bool,
}

/// Sends the emulator's answers to terminal queries (cursor position, device attributes) back to
/// the program, and notes title changes and bells.
pub struct Listener {
    writer: Writer,
    seen: Arc<Mutex<Seen>>,
}

impl EventListener for Listener {
    fn send_event(&self, event: Event) {
        match event {
            Event::PtyWrite(text) => self.writer.reply(text.into_bytes()),
            Event::Title(title) => lock(&self.seen).title = Some(title),
            Event::ResetTitle => lock(&self.seen).title = Some(String::new()),
            Event::Bell => lock(&self.seen).bell = true,
            _ => {}
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
    term: Arc<Mutex<Term<Listener>>>,
    tracker: Arc<Mutex<Tracker>>,
    writer: Writer,
    master: Mutex<Box<dyn MasterPty + Send>>,
    pid: i32,
    /// Set (under its lock) once the program has exited and before it is reaped.
    exited: Arc<Mutex<bool>>,
    /// Bumped whenever the screen may have changed, so a renderer can skip unchanged frames and a
    /// server task can wait for the next change (`subscribe`).
    generation: Arc<watch::Sender<u64>>,
    /// The argv that runs (the login shell's path when `Spec::command` was empty).
    pub command: Vec<String>,
    /// The directory it was started in (None: `$HOME`).
    start_cwd: Option<PathBuf>,
    /// The last OSC 7 pwd (a URL) and title reported.
    reported: Arc<Mutex<Reported>>,
    /// Bytes of output parsed so far.
    output: Arc<AtomicU64>,
    notify: Notify,
}

impl std::fmt::Debug for Terminal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Terminal").field("pid", &self.pid).field("command", &self.command).finish()
    }
}

impl Terminal {
    /// Start `spec` on a new PTY. `notify` gets what the terminal reports, on its reader and waiter
    /// threads; `Notice::Exited` comes last, once the child has been reaped and its output read.
    pub fn spawn(spec: Spec, notify: impl Fn(Notice) + Send + Sync + 'static) -> io::Result<Terminal> {
        let notify: Notify = Arc::new(notify);
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
        let start_cwd = spec.cwd;
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
        let (tx, input) = mpsc::channel::<(Vec<u8>, bool)>();
        let writer = Writer { tx, queued_replies: Arc::default() };
        let queued = writer.queued_replies.clone();
        // Ends once every sender is gone, or when the PTY closes.
        std::thread::spawn(move || {
            input.iter().try_for_each(|(data, reply)| {
                let written = pty_in.write_all(&data);
                if reply {
                    queued.fetch_sub(data.len(), Ordering::SeqCst);
                }
                written
            })
        });
        let config = Config { scrolling_history: SCROLLBACK, ..Config::default() };
        let seen = Arc::new(Mutex::new(Seen::default()));
        let listener = Listener { writer: writer.clone(), seen: seen.clone() };
        let term = Arc::new(Mutex::new(Term::new(config, &Size { rows: ROWS, cols: COLS }, listener)));
        let tracker = Arc::new(Mutex::new(Tracker::default()));

        // Each disconnects when its end is done: the reads at the PTY's EOF, the parsing after the
        // last output's notices.
        let (at_eof, eof) = mpsc::channel::<()>();
        let (parsed, drained) = mpsc::channel::<()>();
        let (reported, output) = (Arc::<Mutex<Reported>>::default(), Arc::new(AtomicU64::new(0)));
        let generation = Arc::new(watch::channel(0).0);
        let reading = Reading {
            term: term.clone(),
            tracker: tracker.clone(),
            seen,
            writer: writer.clone(),
            reported: reported.clone(),
            output: output.clone(),
            changes: generation.clone(),
        };
        let to_reader = notify.clone();
        let to_self = notify.clone();
        std::thread::spawn(move || {
            reading.run(reader, at_eof, &*to_reader);
            drop(parsed);
        });
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
            if eof.recv_timeout(DRAIN_WAIT) == Err(mpsc::RecvTimeoutError::Disconnected) {
                let _ = drained.recv(); // a bounded backlog: it finishes
            }
            notify(Notice::Exited(status));
        });
        Ok(Terminal {
            term,
            tracker,
            writer,
            master: Mutex::new(pair.master),
            pid,
            exited,
            generation,
            command,
            start_cwd,
            reported,
            output,
            notify: to_self,
        })
    }

    pub fn pid(&self) -> i32 {
        self.pid
    }

    /// How many bytes of output the emulator has parsed.
    pub fn output_bytes(&self) -> u64 {
        self.output.load(Ordering::SeqCst)
    }

    /// Queue input for the program; never blocks.
    pub fn write(&self, data: &[u8]) -> io::Result<()> {
        self.writer.input(data.to_vec())
    }

    /// The program-status records (OSC 7501), least recently reported first.
    pub fn program_status(&self) -> Vec<(Option<String>, Record)> {
        lock(&self.tracker).store.records()
    }

    /// Look at the program-status records, most recently reported first, without copying them
    /// (the app reads them on every poll).
    pub fn with_program_status<R>(&self, f: impl FnOnce(&[&Record]) -> R) -> R {
        f(&lock(&self.tracker).store.newest_first())
    }

    /// The user returned to this terminal: drop its `done` and `error` records (spec lifetime) and
    /// report the removal like any other. Don't hold the server's state lock: the notice takes it.
    pub fn acknowledge(&self) {
        let gone = lock(&self.tracker).store.acknowledge();
        if !gone.is_empty() {
            self.generation.send_modify(|g| *g += 1);
        }
        gone.into_iter().for_each(|c| (self.notify)(Notice::ProgramStatus(c)));
    }

    /// Paste `text` the way the program asked for it now (`keys::paste_bytes`).
    pub fn paste(&self, text: &str) -> io::Result<()> {
        let bracketed = lock(&self.term).mode().contains(TermMode::BRACKETED_PASTE);
        self.write(&crate::keys::paste_bytes(text, bracketed))
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

    /// Resize the grid, then the PTY (which signals SIGWINCH), so the program's redraw is parsed
    /// at the new size. At least 1 row and 2 columns (alacritty's wide-char path needs two), at
    /// most `MAX_SIZE`: any client can ask, and a grid's memory grows with rows × columns.
    pub fn resize(&self, rows: u16, cols: u16) -> io::Result<()> {
        let (rows, cols) = (rows.clamp(1, MAX_SIZE.0), cols.clamp(2, MAX_SIZE.1));
        lock(&self.term).resize(Size { rows, cols });
        let size = PtySize { rows, cols, pixel_width: 0, pixel_height: 0 };
        lock(&self.master).resize(size).map_err(pty)?;
        self.generation.send_modify(|g| *g += 1);
        Ok(())
    }

    /// Changes so far; a renderer redraws when this moves.
    pub fn generation(&self) -> u64 {
        *self.generation.borrow()
    }

    /// Wakes on every change to the screen (output parsed, a resize, a status record dropped).
    pub fn subscribe(&self) -> watch::Receiver<u64> {
        self.generation.subscribe()
    }

    /// Read the emulator's state (grid, cursor, modes) under its lock. Keep `f` short: the
    /// reader thread waits for it.
    pub fn with_term<R>(&self, f: impl FnOnce(&Term<Listener>) -> R) -> R {
        f(&lock(&self.term))
    }

    /// Where the program is: its own cwd while it runs, else the last OSC 7 path, else where it
    /// started. The process comes first: an OSC 7 may come from a remote host (ssh).
    pub fn cwd(&self) -> Option<PathBuf> {
        let live = {
            let exited = lock(&self.exited);
            // Under the lock the program is unreaped, so its pid is still its own.
            (!*exited && self.pid > 0).then(|| process_cwd(self.pid)).flatten()
        };
        live.or_else(|| lock(&self.reported).pwd.as_deref().and_then(pwd_path)).or_else(|| self.start_cwd.clone())
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

/// Program-supplied text for events: no control, bidi or invisible characters (they would reach
/// whoever shows the event, docs/spec.md §5), at most `MAX_TEXT` bytes.
fn clean(s: &str) -> String {
    let mut out = String::new();
    for c in strip_hidden(s).chars() {
        if out.len() + c.len_utf8() > MAX_TEXT {
            break;
        }
        out.push(c);
    }
    out
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
#[path = "terminal_tests.rs"]
mod tests;
