//! `leon attach`: the terminal client (docs/architecture.md, "The terminal client"). It is the
//! app's client code (`leon_app::remote`, its keymap and layout) drawn with ratatui instead of
//! GPUI: one more client of the server, so detaching leaves every program running.

mod draw;
mod host;
mod keys;
mod picker;
mod view;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use leon_app::remote::Remote;
use leon_lua::keys::Chord;
use ratatui::crossterm::event::{self, Event, KeyEvent, KeyEventKind};
use serde_json::json;

use crate::client::{Client, Fail, Server};
use crate::commands::resolve;
use draw::Colors;
use keys::Keys;
use picker::{Pick, Picker};

/// Screen-only redraws are at least this far apart, so a flood costs the host a frame per tick
/// (over ssh, a slow link) and never a frame per update.
const FRAME: Duration = Duration::from_millis(16);

enum Msg {
    Input(Event),
    /// The model or a watched screen changed.
    Wake,
    Signal(i32),
    /// The host terminal went away.
    InputGone,
}

enum Mode {
    /// The picker, and the session to go back to on Esc.
    Picker(Picker, Option<String>),
    Attached(String),
}

/// Whether the loop goes on, or how `leon attach` ends.
enum Flow {
    Go,
    Done(Result<(), Fail>),
}

pub fn run(target: Option<&str>, prefix: Option<&str>, autostart: bool, server: &Server) -> Result<(), Fail> {
    use std::io::IsTerminal;
    if !std::io::stdin().is_terminal() || !std::io::stdout().is_terminal() {
        return Err(Fail::Other("leon attach needs a terminal; over ssh, run `ssh -t HOST leon attach`".into()));
    }
    if let Some(p) = prefix {
        keys::one_chord(p).map_err(|e| Fail::Other(format!("--prefix: {e}")))?;
    }
    // any server starts before the watchdog and the signal mask, which it must not inherit
    let mut c = server.connect(autostart)?;
    let start = target.map(|t| resolve_target(&mut c, t)).transpose()?;
    drop(c);
    host::watchdog().map_err(|e| Fail::Other(format!("can't take the terminal: {e}")))?;
    let signals = host::block_signals();
    // over a link, a drop is retried for as long as the host keeps a dropped stream
    let remote = match server.link {
        Some(_) => Remote::connect_retrying(&server.socket, leon_server::resume::HOLD),
        None => Remote::connect(&server.socket),
    }
    .map_err(|e| {
        host::restore();
        Fail::Unreachable(e)
    })?;
    let (tx, rx) = mpsc::channel();
    let mut tui = Tui::new(remote, prefix, start, tx.clone());
    let mut screen = match host::Host::enter() {
        Ok(h) => h,
        Err(e) => {
            host::restore();
            return Err(Fail::Other(format!("can't take the terminal: {e}")));
        }
    };
    if let Some((_, link)) = &server.link {
        link.never_prompt(); // the terminal is the TUI's now
    }
    let woken = Arc::new(AtomicBool::new(false));
    let w = woken.clone();
    let wake = tx.clone();
    // one wake queued at a time: the loop reads the newest model whenever it gets to it
    tui.remote.on_change(move || {
        if !w.swap(true, Ordering::SeqCst) {
            let _ = wake.send(Msg::Wake);
        }
    });
    let input = tx.clone();
    std::thread::spawn(move || loop {
        let m = event::read().map_or(Msg::InputGone, Msg::Input);
        let gone = matches!(m, Msg::InputGone);
        if input.send(m).is_err() || gone {
            return;
        }
    });
    std::thread::spawn(move || loop {
        if tx.send(Msg::Signal(host::wait_signal(&signals))).is_err() {
            return;
        }
    });
    let r = tui.run(&mut screen, &rx, &woken);
    drop(screen); // restores the host
    r
}

/// TARGET: a session, else a block (its tab opens with it focused).
fn resolve_target(c: &mut Client, t: &str) -> Result<Pick, Fail> {
    match resolve(c, "session", t, None) {
        Ok(sid) => Ok(Pick::Session(sid)),
        Err(Fail::Api(e)) if e.is_target_error() => {
            let r = c.call("target.resolve", json!({ "kind": "block", "query": t })).map_err(|_| Fail::Api(e))?;
            let field = |k: &str| r[k].as_str().unwrap_or_default().to_string();
            Ok(Pick::Block(field("session_id"), field("id")))
        }
        Err(f) => Err(f),
    }
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

struct Tui {
    remote: Remote,
    keys: Keys,
    flag: Option<String>,
    colors: Colors,
    mode: Mode,
    /// The prefix was pressed: the next key is the `attach` table's.
    after_prefix: bool,
    /// A config sequence in progress (`ctrl+x>…`).
    pending: Vec<Chord>,
    /// Per block, the size this client last asked for (the size rule, docs/architecture.md).
    asked: std::collections::HashMap<String, (u16, u16)>,
    /// A failed action's message, for the status bar; set from other threads too.
    note: Arc<Mutex<Option<String>>>,
    tx: Sender<Msg>,
}

impl Tui {
    fn new(remote: Remote, flag: Option<&str>, start: Option<Pick>, tx: Sender<Msg>) -> Tui {
        let keys = Keys::new(&remote.call("client.keymap", json!({})).unwrap_or_default(), flag)
            .unwrap_or_else(|_| Keys::new(&json!({}), flag).expect("--prefix was checked"));
        let mut tui = Tui {
            remote,
            keys,
            flag: flag.map(String::from),
            colors: Colors::from_env(),
            mode: Mode::Picker(Picker::default(), None),
            after_prefix: false,
            pending: Vec::new(),
            asked: Default::default(),
            note: Arc::default(),
            tx,
        };
        if let Some(p) = start {
            tui.attach(p);
        }
        tui
    }

    fn run(&mut self, host: &mut host::Host, rx: &Receiver<Msg>, woken: &AtomicBool) -> Result<(), Fail> {
        let io = |e: std::io::Error| Fail::Other(format!("drawing failed: {e}"));
        let (mut dirty, mut last) = (true, Instant::now() - FRAME);
        loop {
            let first = if !dirty {
                rx.recv().ok()
            } else {
                match rx.recv_timeout(FRAME.saturating_sub(last.elapsed())) {
                    Ok(m) => Some(m),
                    Err(RecvTimeoutError::Timeout) => None,
                    Err(RecvTimeoutError::Disconnected) => return Ok(()),
                }
            };
            // everything queued is handled before one draw
            for m in first.into_iter().chain(std::iter::from_fn(|| rx.try_recv().ok())) {
                if let Msg::Wake = m {
                    woken.store(false, Ordering::SeqCst);
                }
                if let Flow::Done(r) = self.handle(m, &mut dirty) {
                    return r;
                }
            }
            if dirty && last.elapsed() >= FRAME {
                dirty = !self.draw(&mut host.term).map_err(io)?;
                last = Instant::now();
            }
        }
    }

    fn handle(&mut self, m: Msg, dirty: &mut bool) -> Flow {
        match m {
            Msg::Wake if !self.remote.alive() => Flow::Done(Err(Fail::Other("the leon server went away".into()))),
            Msg::Wake | Msg::Input(Event::Resize(..)) => {
                *dirty = true;
                Flow::Go
            }
            Msg::Signal(s) => Flow::Done(Err(Fail::Exit(128 + s))),
            Msg::InputGone => Flow::Done(Err(Fail::Other("lost the terminal".into()))),
            Msg::Input(Event::Paste(text)) => {
                self.paste(&text);
                *dirty = true;
                Flow::Go
            }
            Msg::Input(Event::Key(k)) if k.kind != KeyEventKind::Release => self.key(k, dirty),
            Msg::Input(_) => Flow::Go,
        }
    }

    fn key(&mut self, k: KeyEvent, dirty: &mut bool) -> Flow {
        if let Mode::Picker(p, back) = &mut self.mode {
            *dirty = true;
            let rows = picker::rows(&self.remote.model().sessions);
            return match p.key(&k, &rows) {
                picker::Picked::Stay => Flow::Go,
                picker::Picked::Attach(pick) => {
                    self.attach(pick);
                    Flow::Go
                }
                picker::Picked::Leave => match back.take().filter(|s| self.remote.model().session(s).is_some()) {
                    Some(sid) => {
                        self.attach(Pick::Session(sid));
                        Flow::Go
                    }
                    None => Flow::Done(Ok(())),
                },
            };
        }
        self.attached_key(k, dirty)
    }

    /// Attach to what the picker (or TARGET) chose; the keymap is read again, since the config
    /// may have been reloaded since.
    fn attach(&mut self, pick: Pick) {
        let sid = match pick {
            Pick::Session(sid) => sid,
            Pick::Window(sid, wid) => {
                self.act("session.focus_window", json!({ "session_id": sid, "window_id": wid }));
                sid
            }
            Pick::Block(sid, bid) => {
                self.act("session.focus_block", json!({ "session_id": sid, "block_id": bid }));
                sid
            }
            Pick::New => match self.remote.act("session.create", json!({})) {
                Ok(r) => r["session_id"].as_str().unwrap_or_default().to_string(),
                Err(e) => return self.say(e.message),
            },
        };
        if let Ok(km) = self.remote.call("client.keymap", json!({})) {
            if let Ok(k) = Keys::new(&km, self.flag.as_deref()) {
                self.keys = k;
            }
        }
        self.asked.clear();
        (self.after_prefix, self.pending) = (false, Vec::new());
        self.mode = Mode::Attached(sid);
    }

    fn say(&self, msg: impl Into<String>) {
        *lock(&self.note) = Some(msg.into());
    }

    /// Call a method that changes the model; a failure shows on the status bar.
    fn act(&self, method: &str, params: serde_json::Value) -> Option<serde_json::Value> {
        self.remote.act(method, params).map_err(|e| self.say(e.message)).ok()
    }

    /// Run a method on its own connection without waiting (Lua runs up to 30 s); a failure shows
    /// on the status bar.
    fn spawn(&self, method: &str, params: serde_json::Value) {
        let (note, tx) = (self.note.clone(), self.tx.clone());
        self.remote.spawn(method, params, move |r| {
            if let Err(e) = r {
                *lock(&note) = Some(e.message);
                let _ = tx.send(Msg::Wake);
            }
        });
    }
}
