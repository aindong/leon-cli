//! A terminal: a program on a PTY, its output fed through alacritty's emulator on a reader
//! thread so the screen and scrollback can be read back at any time. The reader also tracks OSC
//! 7501 program status and reports title, pwd, bell and status changes; the waiter reports the exit.

use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};
use std::os::fd::FromRawFd;
use std::os::unix::fs::OpenOptionsExt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use alacritty_terminal::event::{Event, EventListener};
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::index::{Column, Point};
use alacritty_terminal::term::{Config, Term, TermMode};
use alacritty_terminal::vte::ansi::Processor;
use portable_pty::{MasterPty, PtySize};
use tokio::sync::watch;

use crate::program_status::{coalesce, strip_hidden, Change, Record, Tracker};

/// The terminal block namespace.
#[path = "terminal_cwd.rs"]
mod cwd;
use cwd::{process_cwd, pwd_path, pwd_url};
#[path = "terminal_foreground.rs"]
mod foreground;
pub use foreground::owned_process_name;
#[path = "terminal_process.rs"]
mod process;
use process::{reap, wait_exit, Registered, Started};
#[path = "terminal_input.rs"]
mod input;
#[path = "terminal_reading.rs"]
mod reading;
#[path = "terminal_start.rs"]
mod start;
#[path = "terminal_status.rs"]
mod status;
#[path = "terminal_view.rs"]
mod view;
use reading::{parsed_by_then, Reading, Reported};
pub use view::Escapes;

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
    /// The program in front changed (`Terminal::check_foreground`): its name, or None.
    Foreground(Option<String>),
}

type Notify = Arc<dyn Fn(Notice) + Send + Sync>;
/// Our own fd on the PTY's slave (`spawn_showing`), until something lets go of it.
type Held = Arc<Mutex<Option<File>>>;

/// Lets go of the held slave fd when dropped.
struct Release(Held);

impl Drop for Release {
    fn drop(&mut self) {
        lock(&self.0).take();
    }
}

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
    /// The command a program registered to resume itself after a restart (`set_resume`).
    resume: Mutex<Registered>,
    /// Lines ever pushed into the scrollback (`View::scrolled`); written under the term's lock.
    scrolled: Arc<AtomicUsize>,
    /// The program set X10 mouse mode (`?9`), which alacritty doesn't track.
    x10: Arc<AtomicBool>,
    /// When input was last queued (`write`): leon's typed notices wait for a quiet pane.
    last_input: Mutex<Option<std::time::Instant>>,
    /// Its program reported a permission prompt, and no person typed here since (`hold_for_person`).
    held: AtomicBool,
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
        Self::spawn_showing(spec, &[], notify)
    }

    /// Like `spawn`, with `shown` fed to the emulator before any of the program's output: on the
    /// screen and in the scrollback, never sent to the program. Escapes in it act, so pass only
    /// text you made inert yourself.
    pub fn spawn_showing(
        spec: Spec,
        shown: &[u8],
        notify: impl Fn(Notice) + Send + Sync + 'static,
    ) -> io::Result<Terminal> {
        Self::spawn_sized(spec, shown, None, notify)
    }

    /// `spawn_showing` at `size` (None: 24x80, clamped as `resize` clamps): the program's first
    /// look at its size is right.
    pub fn spawn_sized(
        spec: Spec,
        shown: &[u8],
        size: Option<(u16, u16)>,
        notify: impl Fn(Notice) + Send + Sync + 'static,
    ) -> io::Result<Terminal> {
        let notify: Notify = Arc::new(notify);
        let (rows, cols) = size.map_or((ROWS, COLS), |(r, c)| (r.clamp(1, MAX_SIZE.0), c.clamp(2, MAX_SIZE.1)));
        let size = PtySize { rows, cols, pixel_width: 0, pixel_height: 0 };
        let pair = start::open_pty(size)?;
        // portable-pty silently falls back to $HOME for a missing dir; refuse instead.
        if let Some(dir) = spec.cwd.as_ref().filter(|d| !d.is_dir()) {
            return Err(io::Error::new(io::ErrorKind::NotFound, format!("no directory {}", dir.display())));
        }
        let tty = pair.master.tty_name().ok_or_else(|| io::Error::other("the PTY has no name"))?;
        // On macOS an exit that closes the PTY's last slave fd waits ~0.6 s for its output to be
        // read, then throws it away: a starved reader missed a fast program's whole output. With
        // ours open the exit waits for the reads. Let go once the program has exited, or when the
        // reads end (nothing would read what the exit waits for).
        let held: Held =
            Arc::new(Mutex::new(Some(OpenOptions::new().read(true).custom_flags(libc::O_NOCTTY).open(&tty)?)));
        let child = start::start(&spec, &tty)?;
        drop(pair.slave);
        let pid = child.id() as i32;
        let started = Started::new(pid, held.clone());
        let command = if spec.command.is_empty() { vec![start::shell(&spec)] } else { spec.command };
        let start_cwd = spec.cwd;

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
        let mut emulator = Term::new(config, &Size { rows, cols }, listener);
        let mut parser: Processor = Processor::new();
        let scrolled = Arc::new(AtomicUsize::new(0));
        view::feed(&mut emulator, &mut parser, &mut Escapes::default(), shown, &scrolled);
        let term = Arc::new(Mutex::new(emulator));
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
            read: Arc::default(),
            changes: generation.clone(),
            scrolled: scrolled.clone(),
            x10: Arc::default(),
            held: held.clone(),
        };
        let (x10, read, parsed_bytes) = (reading.x10.clone(), reading.read.clone(), output.clone());
        let to_reader = notify.clone();
        let to_self = notify.clone();
        std::thread::spawn(move || {
            reading.run(reader, at_eof, &*to_reader);
            drop(parsed);
        });
        #[cfg(test)]
        tests::late_failure(pid)?;
        let exited = Arc::new(Mutex::new(false));
        let done = exited.clone();
        let release = held.clone();
        std::thread::spawn(move || {
            wait_exit(pid);
            lock(&release).take(); // the reads end once whatever it left behind lets go too
            let status = {
                let mut done = lock(&done);
                // The zombie still holds the group ID, so this reaches only what the program left behind.
                // SAFETY: only sends a signal.
                unsafe { libc::kill(-pid, libc::SIGHUP) };
                *done = true; // under the lock, so Drop never signals a group ID that reaping freed
                reap(pid)
            };
            drop(child); // reaped above; dropping std's Child neither waits nor kills
            parsed_by_then(&eof, &drained, &read, &parsed_bytes, DRAIN_WAIT);
            notify(Notice::Exited(status));
        });
        started.disarm(); // the exit thread waits for, reaps and lets go from here on
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
            resume: Mutex::default(),
            scrolled,
            x10,
            last_input: Mutex::new(None),
            held: AtomicBool::new(false),
        })
    }

    pub fn pid(&self) -> i32 {
        self.pid
    }

    /// How many bytes of output the emulator has parsed.
    pub fn output_bytes(&self) -> u64 {
        self.output.load(Ordering::SeqCst)
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
        {
            let mut term = lock(&self.term);
            term.resize(Size { rows, cols });
            // shrinking pushes rows into the scrollback: the oldest kept line keeps a number
            self.scrolled.fetch_max(term.grid().history_size(), Ordering::SeqCst);
        }
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

/// Program-supplied text for events: no control, bidi or invisible characters (they would reach
/// whoever shows the event, docs/agents.md), at most `MAX_TEXT` bytes.
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

fn pty(e: impl std::fmt::Display) -> io::Error {
    io::Error::other(e.to_string())
}

#[cfg(test)]
#[path = "terminal_tests.rs"]
mod tests;
