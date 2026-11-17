//! The reader and parser threads: PTY output into the OSC 7501 tracker and the emulator.

use super::*;

/// The reader thread's share of a terminal.
pub(super) struct Reading {
    pub(super) term: Arc<Mutex<Term<Listener>>>,
    pub(super) tracker: Arc<Mutex<Tracker>>,
    pub(super) seen: Arc<Mutex<Seen>>,
    pub(super) writer: Writer,
    pub(super) reported: Arc<Mutex<Reported>>,
    pub(super) output: Arc<AtomicU64>,
    /// Bytes read from the PTY, counted before they are queued for the parser.
    pub(super) read: Arc<AtomicU64>,
    /// Bumped after each read is fed, so a renderer can skip unchanged frames.
    pub(super) changes: Arc<watch::Sender<u64>>,
    pub(super) scrolled: Arc<AtomicUsize>,
    pub(super) x10: Arc<AtomicBool>,
    pub(super) held: Held,
}

/// The last pwd and title a program reported.
#[derive(Default)]
pub(super) struct Reported {
    pub(super) pwd: Option<String>,
    pub(super) title: Option<String>,
    /// The foreground program last reported (`check_foreground`).
    pub(super) foreground: Option<String>,
}

/// Store what `notice` reports (its title, pwd or foreground), as a snapshot will show it. A server
/// does it under its state lock as it sends the event (`State::notice`); the terminal's own threads
/// do it again once their notify returned, for a terminal no server holds. Never before: a snapshot
/// in between would show a value whose event is still to come.
pub(super) fn remember(reported: &Mutex<Reported>, notice: &Notice) {
    let mut r = lock(reported);
    match notice {
        // an empty title (a reset) gives the tab back its program's name
        Notice::Title(t) => r.title = Some(t.clone()).filter(|t| !t.is_empty()),
        Notice::Pwd(p) => r.pwd = Some(p.clone()),
        Notice::Foreground(f) => r.foreground.clone_from(f),
        _ => {}
    }
}

impl Terminal {
    /// Store what `notice` reports (`remember`); `State::notice` calls it under the state lock.
    pub(crate) fn remember(&self, notice: &Notice) {
        remember(&self.reported, notice);
    }

    /// The last title the program set (OSC 0/2), cleaned as the `title_changed` event carries it.
    pub fn title(&self) -> Option<String> {
        lock(&self.reported).title.clone()
    }

    /// Start with a saved title (from before a restart) until the program sets one; one it set
    /// already wins. Cleaned again, since it goes on screen.
    pub fn restore_title(&self, title: &str) {
        let title = clean(title);
        let mut r = lock(&self.reported);
        if r.title.is_none() && !title.is_empty() {
            r.title = Some(title);
        }
    }
}

impl Reading {
    /// Feed output to the tracker and emulator until the PTY closes, reporting per batch. A thread
    /// of its own does the reads. Takes the term and tracker locks only while feeding, never while
    /// notifying.
    pub(super) fn run(self, mut reader: Box<dyn Read + Send>, at_eof: mpsc::Sender<()>, notify: &dyn Fn(Notice)) {
        let (tx, reads) = mpsc::sync_channel::<Vec<u8>>(QUEUED_READS);
        let (held, read) = (self.held.clone(), self.read.clone());
        std::thread::spawn(move || {
            let _at_eof = at_eof; // dropped when reading ends
            let _release = Release(held); // nothing reads now, so the program's exit mustn't wait
            let mut buf = vec![0u8; READ_SIZE];
            loop {
                let n = match reader.read(&mut buf) {
                    // macOS reports a closed PTY as EIO rather than EOF.
                    Ok(0) | Err(_) => return,
                    Ok(n) => n,
                };
                read.fetch_add(n as u64, Ordering::SeqCst);
                if tx.send(buf[..n].to_vec()).is_err() {
                    return;
                }
            }
        });
        let mut parser: Processor = Processor::new();
        // The last title and pwd reported: shells resend them at every prompt.
        let (mut title, mut pwd) = (None, None);
        let mut escapes = super::view::Escapes::default();
        while let Ok(mut batch) = reads.recv() {
            while batch.len() < BATCH {
                match reads.try_recv() {
                    Ok(more) => batch.extend_from_slice(&more),
                    Err(_) => break,
                }
            }
            let fed = {
                let mut term = lock(&self.term);
                lock(&self.tracker).feed(&batch, |bytes| {
                    super::view::feed(&mut term, &mut parser, &mut escapes, bytes, &self.scrolled)
                })
            };
            self.x10.store(escapes.x10(), Ordering::SeqCst);
            self.output.fetch_add(batch.len() as u64, Ordering::SeqCst);
            self.changes.send_modify(|g| *g += 1);
            if !fed.reply.is_empty() {
                self.writer.reply(fed.reply);
            }
            let seen = std::mem::take(&mut *lock(&self.seen));
            if let Some(new) = seen.title.map(|t| clean(&t)).filter(|t| title.as_ref() != Some(t)) {
                title = Some(new.clone());
                notify(Notice::Title(new.clone()));
                remember(&self.reported, &Notice::Title(new));
            }
            let new_pwd = fed.pwd.and_then(|p| pwd_url(&p));
            if let Some(new) = new_pwd.filter(|p| pwd.as_ref() != Some(p)) {
                pwd = Some(new.clone());
                notify(Notice::Pwd(new.clone()));
                remember(&self.reported, &Notice::Pwd(new));
            }
            if seen.bell {
                notify(Notice::Bell);
            }
            coalesce(fed.changes).into_iter().for_each(|c| notify(Notice::ProgramStatus(c)));
        }
        let gone = lock(&self.tracker).store.process_exited();
        gone.into_iter().for_each(|c| notify(Notice::ProgramStatus(c)));
    }
}

/// Wait for what the exit report needs: the PTY's EOF and every read parsed, or, when no EOF comes
/// within `wait` (a job holds the PTY, or the reader is behind), what was read by then parsed.
pub(super) fn parsed_by_then(
    eof: &mpsc::Receiver<()>,
    drained: &mpsc::Receiver<()>,
    read: &AtomicU64,
    parsed: &AtomicU64,
    wait: Duration,
) {
    if eof.recv_timeout(wait) == Err(mpsc::RecvTimeoutError::Disconnected) {
        let _ = drained.recv(); // a bounded backlog: it finishes
        return;
    }
    let read = read.load(Ordering::SeqCst);
    // polled: the parser has no per-batch signal for a plain thread; it ends if the parser does
    let tick = Duration::from_millis(5);
    while parsed.load(Ordering::SeqCst) < read && drained.recv_timeout(tick) == Err(mpsc::RecvTimeoutError::Timeout) {}
}
