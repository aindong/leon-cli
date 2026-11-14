//! The app's link to a leon server over its socket (docs/architecture.md, "The app attaches").
//! Three connections: one for the UI's calls (keys, layout changes), one that refetches the model,
//! and one that receives events and screens. A thread reads the last: a screen line patches that
//! block's frame, and an event means the model is refetched once the lines that came with it are
//! read. Calls that may run long (Lua actions) get a connection of their own on a thread.
//! Over a host's link (`connect_retrying`) a dropped event connection is retried with backoff,
//! every connection is opened again when it comes back, and the event stream resumes after the
//! last seq seen (remote_link.rs).

use std::collections::HashMap;
use std::io::{self, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use leon_core::program_status::Record;
use leon_core::screen::{self, Frame, Update};
use leon_core::{ApiError, Window};
use serde::Deserialize;
use serde_json::{json, Value};

#[path = "remote_fetch.rs"]
mod fetch_model;
#[path = "remote_link.rs"]
mod link;
use fetch_model::{fetch, read_line};
use link::Job;
pub use link::LinkState;

/// How long a UI call waits for its reply before giving up (the server answers in microseconds).
const CALL_TIMEOUT: Duration = Duration::from_secs(5);
/// How long the first call over a link waits.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);
/// One session as the app shows it.
#[derive(Debug, Clone)]
pub struct Session {
    pub id: String,
    pub label: Option<String>,
    pub revision: u64,
    /// Index of the active window.
    pub active: usize,
    pub windows: Vec<Window>,
    pub blocks: Vec<Block>,
}

#[derive(Debug, Clone)]
pub struct Block {
    pub id: String,
    pub window_id: String,
    pub command: Vec<String>,
    /// The title the program set, and where it is (`session.list_blocks`).
    pub title: Option<String>,
    pub cwd: Option<String>,
    /// Program-status records, least recently reported first.
    pub status: Vec<Record>,
}

impl Block {
    /// The records most recently reported first, as `status::indicator_of` takes them.
    pub fn newest_first(&self) -> Vec<&Record> {
        self.status.iter().rev().collect()
    }
}

/// The server's sessions and the watched blocks' screens, as last received.
#[derive(Default)]
pub struct Model {
    pub sessions: Vec<Session>,
    pub screens: HashMap<String, Arc<Frame>>,
    /// Per block, how many screen updates came: what a redraw check compares.
    pub screen_updates: HashMap<String, u64>,
}

impl Model {
    pub fn session(&self, id: &str) -> Option<&Session> {
        self.sessions.iter().find(|s| s.id == id)
    }

    /// A block and the session it belongs to.
    pub fn block(&self, id: &str) -> Option<(&Session, &Block)> {
        self.sessions.iter().find_map(|s| s.blocks.iter().find(|b| b.id == id).map(|b| (s, b)))
    }
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

fn unreachable(e: impl std::fmt::Display) -> ApiError {
    ApiError::new(503, "unreachable", format!("the leon server can't be reached: {e}"))
}

/// One NDJSON connection used for request and reply only.
struct Conn {
    r: BufReader<UnixStream>,
    id: u64,
}

impl Conn {
    fn open(socket: &Path, timeout: Option<Duration>) -> io::Result<Conn> {
        let s = UnixStream::connect(socket)?;
        s.set_read_timeout(timeout)?;
        Ok(Conn { r: BufReader::new(s), id: 0 })
    }

    fn call(&mut self, method: &str, params: Value) -> Result<Value, ApiError> {
        self.id += 1;
        let mut line = serde_json::to_vec(&json!({ "id": self.id, "method": method, "params": params })).expect("json");
        line.push(b'\n');
        self.r.get_mut().write_all(&line).map_err(unreachable)?;
        loop {
            let mut reply = String::new();
            match read_line(&mut self.r, &mut reply) {
                Ok(0) => return Err(unreachable("the connection closed")),
                Err(e) => return Err(unreachable(e)),
                Ok(_) => {}
            }
            let mut v: Value = serde_json::from_str(&reply).map_err(unreachable)?;
            if v["id"] != self.id {
                continue; // a late reply to a call that timed out
            }
            return match v.get_mut("error").filter(|e| !e.is_null()) {
                Some(e) => Err(serde_json::from_value(e.take()).unwrap_or_else(unreachable)),
                None => Ok(v["result"].take()),
            };
        }
    }
}

/// What the reader thread and the UI share.
struct Shared {
    socket: PathBuf,
    /// How long a re-attach keeps trying (over a link); None gives up at once.
    retry: Option<Duration>,
    /// The UI's calls. Here, so a re-attach can replace it too.
    calls: Mutex<Conn>,
    model: Mutex<Model>,
    /// Refetches go here, one at a time and never through the UI's connection, so a refetch can't
    /// delay a keystroke and an older refetch can't overwrite a newer one.
    fetch: Mutex<Conn>,
    /// Run after every change to `model`.
    wakers: Mutex<Vec<Box<dyn Fn() + Send>>>,
    /// The event connection's write side, for `block.watch`.
    events: Mutex<UnixStream>,
    watched: Mutex<Vec<String>>,
    alive: AtomicBool,
    closing: AtomicBool,
    /// Over a link: whether the connections are up.
    link: Mutex<LinkState>,
    /// The resumable stream's token, the last event seq seen, and how often the seq skipped.
    resume: Mutex<Option<String>>,
    seq: AtomicU64,
    gaps: AtomicU64,
    resumes: AtomicU64,
    reattaches: AtomicU64,
    /// When the event connection last brought a line, and when the watchdog pinged it.
    heard: Mutex<Instant>,
    ping: Mutex<Option<Instant>>,
}

impl Shared {
    fn bump(&self) {
        lock(&self.wakers).iter().for_each(|w| w());
    }

    /// A call on the UI's connection, replaced for the next call when it failed (`Remote::call`).
    fn call(&self, method: &str, params: Value) -> Result<Value, ApiError> {
        let mut c = lock(&self.calls);
        let r = c.call(method, params);
        if r.as_ref().is_err_and(|e| e.code == "unreachable") {
            if let Ok(fresh) = Conn::open(&self.socket, Some(CALL_TIMEOUT)) {
                *c = fresh;
            }
        }
        r
    }

    /// Fetch the whole model and replace it; screens of blocks that are gone are dropped.
    fn refresh(&self) -> Result<(), ApiError> {
        let mut c = lock(&self.fetch);
        let sessions = fetch(&mut c)?;
        // screens go with the watch set (`Remote::watch`), not here: a refetch can skip a session
        lock(&self.model).sessions = sessions;
        drop(c);
        self.bump();
        Ok(())
    }

    /// A stream attached afresh: its token, and the seq its snapshot sits at.
    fn restart_stream(&self, snapshot: &Value) {
        *lock(&self.resume) = snapshot["resume"].as_str().map(String::from);
        self.seq.store(snapshot["seq"].as_u64().unwrap_or(0), Ordering::SeqCst);
    }

    /// An event's seq: one more than the last, or a gap is counted.
    fn saw(&self, seq: u64) {
        if self.seq.swap(seq, Ordering::SeqCst) + 1 != seq {
            self.gaps.fetch_add(1, Ordering::SeqCst);
        }
    }

    fn send_watch(&self, ids: &[String]) -> io::Result<()> {
        let mut line =
            serde_json::to_vec(&json!({ "id": 0, "method": "block.watch", "params": { "block_ids": ids } }))?;
        line.push(b'\n');
        lock(&self.events).write_all(&line)
    }
}

/// A line on the event connection, read into types rather than a `Value` tree: a server (maybe a
/// box we don't control) gets about a byte of memory per byte it sends, not dozens. Unknown and
/// unused fields are skipped without being kept.
#[derive(Deserialize)]
struct Incoming {
    screen: Option<Pushed>,
    event: Option<serde::de::IgnoredAny>,
    seq: Option<u64>,
    error: Option<Refusal>,
}

#[derive(Deserialize)]
struct Refusal {
    message: Option<String>,
}

/// A screen line's body: `Update`'s fields spelled out, since serde's `flatten` would first buffer
/// the whole line as a tree.
#[derive(Deserialize)]
struct Pushed {
    block_id: String,
    full: bool,
    lines: usize,
    cols: usize,
    rows: Vec<screen::Row>,
    cursor: Option<screen::Cursor>,
    #[serde(default)]
    app_cursor: bool,
    #[serde(default)]
    bracketed_paste: bool,
    #[serde(default)]
    scrolled: usize,
    #[serde(default)]
    history: usize,
    #[serde(default)]
    alt_screen: bool,
    #[serde(default)]
    alternate_scroll: bool,
    #[serde(default)]
    mouse: Option<leon_core::mouse::Report>,
}

impl Pushed {
    fn update(self) -> Update {
        let view = screen::View {
            scrolled: self.scrolled,
            history: self.history,
            alt_screen: self.alt_screen,
            alternate_scroll: self.alternate_scroll,
            mouse: self.mouse,
        };
        let (full, lines, cols, rows, cursor) = (self.full, self.lines, self.cols, self.rows, self.cursor);
        Update {
            full,
            lines,
            cols,
            rows,
            cursor,
            app_cursor: self.app_cursor,
            bracketed_paste: self.bracketed_paste,
            view,
        }
    }
}

/// Open an event connection and attach it to every event; returns its reader, its write side
/// and the snapshot (with a `resume` token when `resumable`).
fn attach(socket: &Path, resumable: bool, wait: Duration) -> io::Result<(BufReader<UnixStream>, UnixStream, Value)> {
    let mut c = Conn::open(socket, Some(wait))?;
    let p = json!({ "event_scope": "all", "resumable": resumable });
    let snapshot = c.call("session.attach_all", p).map_err(|e| io::Error::other(e.message))?;
    c.r.get_ref().set_read_timeout(None)?;
    let write = c.r.get_ref().try_clone()?;
    Ok((c.r, write, snapshot))
}

/// The reader thread: apply screens, refetch after events, re-attach after a drop while the
/// server still answers, and say the server is gone when it doesn't.
fn read_events(shared: Arc<Shared>, mut r: BufReader<UnixStream>) {
    loop {
        let mut stale = false;
        loop {
            let mut line = String::new();
            let read = read_line(&mut r, &mut line);
            if matches!(read, Ok(0) | Err(_)) {
                if shared.closing.load(Ordering::SeqCst) {
                    return;
                }
                // a line past the cap would come again after a re-attach: give up on this server
                let refused = matches!(&read, Err(e) if e.kind() == io::ErrorKind::InvalidData);
                if refused {
                    eprintln!("leon: the server sent a line past {} MiB; disconnecting", fetch_model::MAX_LINE >> 20);
                }
                match reattach(&shared).filter(|_| !refused) {
                    Some(again) => (r, stale) = (again, true),
                    None => {
                        shared.alive.store(false, Ordering::SeqCst);
                        shared.bump();
                        return;
                    }
                }
                break;
            }
            *lock(&shared.heard) = Instant::now();
            let Ok(v) = serde_json::from_str::<Incoming>(&line) else { continue };
            if let Some(p) = v.screen {
                // a line still on its way for a block no longer watched would revive a stale copy
                if lock(&shared.watched).contains(&p.block_id) {
                    let mut m = lock(&shared.model);
                    *m.screen_updates.entry(p.block_id.clone()).or_default() += 1;
                    let f = m.screens.entry(p.block_id.clone()).or_default();
                    screen::apply(Arc::make_mut(f), p.update());
                }
            } else if v.event.is_some() {
                if let Some(seq) = v.seq {
                    shared.saw(seq);
                }
                stale = true;
            } else if let Some(e) = v.error {
                eprintln!("leon: the server refused a watch: {}", e.message.unwrap_or_default());
            }
            if !r.buffer().contains(&b'\n') {
                break; // nothing more already read: show what came
            }
        }
        if stale {
            let _ = shared.refresh();
        } else {
            shared.bump();
        }
    }
}

/// A new event connection when the server still answers (it dropped a connection that fell too
/// far behind, or a link to it came back), with the watched blocks watched again.
fn reattach(shared: &Shared) -> Option<BufReader<UnixStream>> {
    let (r, w) = match shared.retry {
        None => {
            lock(&shared.fetch).call("server.version", json!({})).ok()?;
            let (r, w, snapshot) = attach(&shared.socket, false, CALL_TIMEOUT).ok()?;
            shared.restart_stream(&snapshot);
            (r, w)
        }
        Some(limit) => link::relink(shared, limit)?,
    };
    // held while sending, as `Remote::watch` holds it, so a newer set can't be overtaken
    let watched = lock(&shared.watched);
    *lock(&shared.events) = w;
    if shared.closing.load(Ordering::SeqCst) {
        let _ = lock(&shared.events).shutdown(std::net::Shutdown::Both); // dropped meanwhile
        return None;
    }
    lock(&shared.model).screens.clear(); // the new connection starts with whole screens
    shared.send_watch(&watched).ok()?;
    Some(r)
}

/// A resize the app asked for: the size, and the size the screen had then.
type Asked = ((u16, u16), (usize, usize));

pub struct Remote {
    shared: Arc<Shared>,
    /// Over a link, the window's calls go through here (`link::run_queue`).
    queue: Option<Sender<Job>>,
    /// Per block: the size last asked for and the size the screen had then (`fit`).
    asked: Mutex<HashMap<String, Asked>>,
}

impl Remote {
    /// Attach to the server on `socket` and read its model; fails when nothing answers.
    pub fn connect(socket: &Path) -> Result<Remote, String> {
        Remote::open(socket, None)
    }

    /// `connect` for a host's link (leon_hosts::Link): when the connections drop, keep trying
    /// to get them back for up to `limit` before saying the server is gone.
    pub fn connect_retrying(socket: &Path, limit: Duration) -> Result<Remote, String> {
        Remote::open(socket, Some(limit))
    }

    fn open(socket: &Path, retry: Option<Duration>) -> Result<Remote, String> {
        // Keys go to whatever answers, so only to a socket in a dir only we can use (the server
        // binds only in one): a socket in an open dir could be anyone's.
        leon_core::paths::ensure_private_dir(socket.parent().unwrap_or(Path::new("."))).map_err(|e| e.to_string())?;
        let no_server = |e: &dyn std::fmt::Display| format!("no leon server at {}: {e}", socket.display());
        // the first call over a link waits for ssh to log in and the host's server to start
        let first = if retry.is_some() { CONNECT_TIMEOUT } else { CALL_TIMEOUT };
        let mut calls = Conn::open(socket, Some(first)).map_err(|e| no_server(&e))?;
        calls.call("server.version", json!({})).map_err(|e| no_server(&e.message))?;
        calls.r.get_ref().set_read_timeout(Some(CALL_TIMEOUT)).map_err(|e| no_server(&e))?;
        let fetch = Conn::open(socket, Some(first)).map_err(|e| no_server(&e))?;
        let (r, events, snapshot) = attach(socket, retry.is_some(), first).map_err(|e| no_server(&e))?;
        let shared = Arc::new(Shared {
            socket: socket.to_path_buf(),
            retry,
            calls: Mutex::new(calls),
            model: Mutex::default(),
            fetch: Mutex::new(fetch),
            wakers: Mutex::default(),
            events: Mutex::new(events),
            watched: Mutex::default(),
            alive: AtomicBool::new(true),
            closing: AtomicBool::new(false),
            link: Mutex::new(LinkState::Up),
            resume: Mutex::default(),
            seq: AtomicU64::new(0),
            gaps: AtomicU64::new(0),
            resumes: AtomicU64::new(0),
            reattaches: AtomicU64::new(0),
            heard: Mutex::new(Instant::now()),
            ping: Mutex::default(),
        });
        shared.restart_stream(&snapshot);
        shared.refresh().map_err(|e| no_server(&e.message))?;
        lock(&shared.fetch).r.get_ref().set_read_timeout(Some(CALL_TIMEOUT)).map_err(|e| no_server(&e))?;
        let reader = shared.clone();
        std::thread::Builder::new()
            .name("leon-events".into())
            .spawn(move || read_events(reader, r))
            .map_err(|e| e.to_string())?;
        let queue = match retry {
            None => None,
            Some(_) => {
                let (tx, jobs) = std::sync::mpsc::channel();
                let (q, w) = (Arc::downgrade(&shared), Arc::downgrade(&shared));
                let spawn =
                    |name: &str, f: Box<dyn FnOnce() + Send>| std::thread::Builder::new().name(name.into()).spawn(f);
                spawn("leon-link-calls", Box::new(move || link::run_queue(q, jobs))).map_err(|e| e.to_string())?;
                spawn("leon-link-watchdog", Box::new(move || link::watchdog(w))).map_err(|e| e.to_string())?;
                Some(tx)
            }
        };
        Ok(Remote { shared, queue, asked: Mutex::default() })
    }

    /// The model as last received. Hold it briefly: the reader thread waits for it.
    pub fn model(&self) -> MutexGuard<'_, Model> {
        lock(&self.shared.model)
    }

    /// Call a method and wait for the reply (methods that answer at once; not `block.wait`). A
    /// connection that failed (the server closed it, a reply never came) is replaced for the next
    /// call; this one is not sent again, since it may have been done.
    pub fn call(&self, method: &str, params: Value) -> Result<Value, ApiError> {
        self.shared.call(method, params)
    }

    /// Call a method that changes the model, then refetch it, so what the app shows next
    /// includes the change.
    pub fn act(&self, method: &str, params: Value) -> Result<Value, ApiError> {
        let r = self.call(method, params);
        let _ = self.shared.refresh();
        r
    }

    /// Run a method on a connection of its own without waiting: config actions and key bindings
    /// run Lua for up to 30 s, which must not stall the UI. `done` gets the reply on that thread.
    pub fn spawn(&self, method: &str, params: Value, done: impl FnOnce(Result<Value, ApiError>) + Send + 'static) {
        let (socket, method) = (self.shared.socket.clone(), method.to_string());
        std::thread::spawn(move || {
            done(Conn::open(&socket, None).map_err(unreachable).and_then(|mut c| c.call(&method, params)))
        });
    }

    /// Type `bytes` into a block.
    pub fn write(&self, session_id: &str, block_id: &str, bytes: &[u8]) {
        let data = String::from_utf8_lossy(bytes);
        let args = json!({ "data": data });
        let p = json!({ "session_id": session_id, "block_id": block_id, "method": "dev.leon.terminal.write", "args": args });
        self.send("block.method", p);
    }

    pub fn resize(&self, session_id: &str, block_id: &str, rows: u16, cols: u16) {
        let args = json!({ "rows": rows, "cols": cols });
        let p = json!({ "session_id": session_id, "block_id": block_id, "method": "dev.leon.terminal.resize", "args": args });
        self.send("block.method", p);
    }

    /// Resize a block to (rows, cols) unless that was already asked for while its screen had the
    /// size it has now (each ask is a round trip, and the screen follows a moment later).
    pub fn fit(&self, session_id: &str, block_id: &str, rows: u16, cols: u16) {
        let shown = self.model().screens.get(block_id).map_or((0, 0), |f| (f.lines, f.cols));
        if shown == (rows as usize, cols as usize) {
            return;
        }
        let ask = ((rows, cols), shown);
        if lock(&self.asked).insert(block_id.to_string(), ask) != Some(ask) {
            self.resize(session_id, block_id, rows, cols);
        }
    }

    /// Have the server push these blocks' screens (and stop the others'). The copies of the
    /// others' screens go too: they would go stale, and `fit` must not trust them; the server
    /// sends a whole screen when one is watched again.
    pub fn watch(&self, ids: Vec<String>) {
        let mut watched = lock(&self.shared.watched);
        if *watched != ids {
            if let Err(e) = self.shared.send_watch(&ids) {
                eprintln!("leon: can't watch the panes: {e}");
            }
            let mut m = self.model();
            m.screens.retain(|id, _| ids.contains(id));
            m.screen_updates.retain(|id, _| ids.contains(id));
            drop(m);
            *watched = ids;
        }
    }

    /// Whether the server still answers (false once it stopped).
    pub fn alive(&self) -> bool {
        self.shared.alive.load(Ordering::SeqCst)
    }

    /// Run `f` (on the reader thread) after every change to the model.
    pub fn on_change(&self, f: impl Fn() + Send + 'static) {
        lock(&self.shared.wakers).push(Box::new(f));
    }
}

impl Drop for Remote {
    /// Detach: the connections close, and the server and its programs carry on.
    fn drop(&mut self) {
        self.shared.closing.store(true, Ordering::SeqCst);
        let _ = lock(&self.shared.events).shutdown(std::net::Shutdown::Both);
        lock(&self.shared.wakers).clear();
    }
}
