//! A fake This Mac server for the window: it answers the handshake and lists one session with one
//! pane `block:0`, logs every other call in the order it arrives, and answers those only when
//! told to (`Answer::All`), so a test can hold the window's calls unanswered or check their order.
//! The window's housekeeping (a resize, the geometry, a focus event) is always answered: an
//! unanswered call holds the ordered queue for its timeout, and these come first.
#![allow(dead_code)]

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use gpui::{px, size, Entity, TestAppContext, VisualTestContext};
use leon_app::view::{self, Workspace};
use serde_json::{json, Value};

#[derive(Clone, Copy, PartialEq)]
pub enum Answer {
    /// Only the handshake, the model and a selection's range.
    Model,
    /// Every call (`{}`, a pwd of `/tmp`).
    All,
}

pub struct Fake {
    _dir: tempfile::TempDir,
    pub socket: PathBuf,
    /// The calls past the handshake and the model, in arrival order: a `block.method` by its
    /// method (`dev.leon.terminal.write`), a `block.event` by its event's type (`block.event paste`).
    pub log: Arc<Mutex<Vec<String>>>,
    /// Every connection made, and whether the server hung up.
    conns: Arc<Mutex<(Vec<UnixStream>, bool)>>,
}

impl Fake {
    /// Close every connection and refuse new ones: a call waiting for a reply fails at once.
    pub fn hang_up(&self) {
        let mut c = self.conns.lock().unwrap();
        c.1 = true;
        c.0.iter().for_each(|s| drop(s.shutdown(std::net::Shutdown::Both)));
    }
}

fn reply(id: &Value, result: Value) -> Vec<u8> {
    let mut l = serde_json::to_vec(&json!({ "id": id, "result": result })).unwrap();
    l.push(b'\n');
    l
}

fn connection(s: UnixStream, answer: Answer, log: Arc<Mutex<Vec<String>>>) {
    let mut w = s.try_clone().unwrap();
    for line in BufReader::new(s).lines() {
        let Ok(line) = line else { return };
        let Ok(v) = serde_json::from_str::<Value>(&line) else { continue };
        let (id, p, method) = (&v["id"], &v["params"], v["method"].as_str().unwrap_or_default());
        let block = json!({ "block_id": "block:0", "window_id": "window:0", "command": ["sh"] });
        let window = json!({ "window_id": "window:0", "active": true, "focused_block_id": "block:0",
                             "layout": { "block": { "block_id": "block:0" } } });
        let inner = p["method"].as_str().unwrap_or_default();
        let out = match method {
            "server.version" => Some(json!({ "version": "fake" })),
            "session.attach_all" => Some(json!({ "seq": 0, "sessions": [] })),
            "session.list" => Some(json!({ "sessions": [{ "session_id": "session:0" }] })),
            "session.view" => Some(json!({ "session_id": "session:0", "windows": [window] })),
            "session.list_blocks" => Some(json!({ "blocks": [block] })),
            "block.method" if inner == "dev.leon.terminal.program_status" => Some(json!({ "records": [] })),
            "block.method" if inner == "dev.leon.terminal.selection" && p["args"]["text"] == false => {
                Some(json!({ "start": { "line": 0, "col": 0 }, "end": { "line": 0, "col": 3 } }))
            }
            // the window's own asks on their own connections, and the watch on the events one
            // a blank 3x20 screen for the pane once it is watched (a selection needs rows)
            "block.watch" => {
                let screen =
                    json!({ "screen": { "block_id": "block:0", "full": true, "lines": 3, "cols": 20, "rows": [] } });
                let mut l = serde_json::to_vec(&screen).unwrap();
                l.push(b'\n');
                let _ = w.write_all(&l);
                None
            }
            "client.keymap" | "server.settings" => None,
            "client.geometry" => Some(json!({})),
            _ => {
                let name = match method {
                    "block.method" => inner.to_string(),
                    "block.event" => format!("block.event {}", p["event"]["type"].as_str().unwrap_or_default()),
                    m => m.to_string(),
                };
                let chores = ["dev.leon.terminal.resize", "block.event focus"].contains(&name.as_str());
                log.lock().unwrap().push(name);
                let pwd = inner == "dev.leon.terminal.pwd";
                (answer == Answer::All || chores).then(|| if pwd { json!({ "path": "/tmp" }) } else { json!({}) })
            }
        };
        if let Some(out) = out {
            if w.write_all(&reply(id, out)).is_err() {
                return;
            }
        }
    }
}

pub fn serve(answer: Answer) -> Fake {
    std::env::set_var("LEON_SPAWN_WAIT_SECS", "2"); // the keymap's spawned ask, on its own thread
    let dir =
        tempfile::Builder::new().permissions(std::os::unix::fs::PermissionsExt::from_mode(0o700)).tempdir().unwrap();
    let socket = dir.path().join("server.sock");
    let listener = UnixListener::bind(&socket).unwrap();
    let log = Arc::new(Mutex::new(Vec::new()));
    let conns: Arc<Mutex<(Vec<UnixStream>, bool)>> = Arc::default();
    let (l, c) = (log.clone(), conns.clone());
    std::thread::spawn(move || {
        for s in listener.incoming().flatten() {
            let mut kept = c.lock().unwrap();
            if kept.1 {
                continue; // hung up: dropped at once
            }
            kept.0.extend(s.try_clone().ok());
            drop(kept);
            let l = l.clone();
            std::thread::spawn(move || connection(s, answer, l));
        }
    });
    Fake { _dir: dir, socket, log, conns }
}

/// A window on the fake.
pub fn open_fake<'a>(cx: &'a mut TestAppContext, f: &Fake) -> (Entity<Workspace>, &'a mut VisualTestContext) {
    std::env::set_var("LEON_TAILSCALE", "/nonexistent/tailscale");
    cx.update(view::init);
    let socket = f.socket.clone();
    let (ws, cx) = cx.add_window_view(move |window, cx| {
        let remote = Rc::new(leon_app::remote::Remote::connect(&socket).expect("the fake answers the handshake"));
        Workspace::with_settings(remote, None, window, cx)
    });
    cx.simulate_resize(size(px(1200.), px(800.)));
    cx.run_until_parked();
    (ws, cx)
}
