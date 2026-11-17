//! Audit A1, A5, A7–A10: the app's connection to a server it doesn't control. A fake server
//! answers the handshake, then misbehaves per method: a call has one deadline however many
//! non-reply lines or bytes come (A1), a server that stops reading can't hold a write (A7), a
//! spawned call ends (A9), a bad UTF-8 line doesn't end the link (A8), and what a session lists is
//! capped (A5) and checked (A10).

use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use leon_app::remote::Remote;
use serde_json::{json, Value};

/// What a fake server lists.
#[derive(Clone, Default)]
struct Listing {
    sessions: usize,
    windows: usize,
    blocks: usize,
    /// A layout to give the first window.
    layout: Option<Value>,
    /// Bytes sent on the event connection right after the attach reply.
    events: Vec<u8>,
}

struct Fake {
    _dir: tempfile::TempDir,
    socket: PathBuf,
}

fn reply(id: &Value, result: Value) -> Vec<u8> {
    let mut l = serde_json::to_vec(&json!({ "id": id, "result": result })).unwrap();
    l.push(b'\n');
    l
}

fn serve(listing: Listing) -> Fake {
    // a spawned call's wait, 2 s for every test here (the Lua limit and 5 s by default): set once,
    // before any test's connection starts
    static SPAWN_WAIT: std::sync::Once = std::sync::Once::new();
    SPAWN_WAIT.call_once(|| std::env::set_var("LEON_SPAWN_WAIT_SECS", "2"));
    let dir = tempfile::Builder::new().permissions(std::fs::Permissions::from_mode(0o700)).tempdir().unwrap();
    let socket = dir.path().join("server.sock");
    let listener = UnixListener::bind(&socket).unwrap();
    std::thread::spawn(move || {
        for s in listener.incoming().flatten() {
            let listing = listing.clone();
            std::thread::spawn(move || connection(s, &listing));
        }
    });
    Fake { _dir: dir, socket }
}

fn connection(s: UnixStream, l: &Listing) {
    let mut w = s.try_clone().unwrap();
    let mut r = BufReader::new(s);
    loop {
        let mut line = String::new();
        if r.read_line(&mut line).unwrap_or(0) == 0 {
            return;
        }
        let Ok(v) = serde_json::from_str::<Value>(&line) else { continue };
        let id = &v["id"];
        let sid = |i: usize| format!("session:{i}");
        let out = match v["method"].as_str().unwrap_or_default() {
            "server.version" => reply(id, json!({ "version": "fake" })),
            "session.attach_all" => {
                let mut out = reply(id, json!({ "seq": 0, "sessions": [] }));
                out.extend_from_slice(&l.events);
                out
            }
            "session.list" => {
                let all: Vec<Value> = (0..l.sessions).map(|i| json!({ "session_id": sid(i) })).collect();
                reply(id, json!({ "sessions": all }))
            }
            "session.view" => {
                let windows: Vec<Value> = (0..l.windows)
                    .map(|i| {
                        let layout = if i == 0 { l.layout.clone() } else { None };
                        json!({ "window_id": format!("window:{i}"), "active": i == 0, "layout": layout })
                    })
                    .collect();
                reply(id, json!({ "session_id": v["params"]["session_id"], "windows": windows }))
            }
            "session.list_blocks" => {
                let blocks: Vec<Value> = (0..l.blocks)
                    .map(|i| json!({ "block_id": format!("block:{i}"), "window_id": format!("window:{}", i % l.windows.max(1)), "command": ["sh"] }))
                    .collect();
                reply(id, json!({ "blocks": blocks }))
            }
            "block.method" => reply(id, json!({ "records": [] })),
            // the reply, then this connection is never read again
            "test.deafen" => {
                let _ = w.write_all(&reply(id, json!({})));
                loop {
                    std::thread::sleep(Duration::from_secs(60));
                }
            }
            // never a reply: a line that isn't one every 50 ms
            "test.flood" => loop {
                if w.write_all(b"{}\n").is_err() {
                    return;
                }
                std::thread::sleep(Duration::from_millis(50));
            },
            // never a reply: a line that never ends, a byte at a time
            "test.trickle" => {
                let _ = w.write_all(b"{\"x\":\"");
                loop {
                    if w.write_all(b"a").is_err() {
                        return;
                    }
                    std::thread::sleep(Duration::from_millis(200));
                }
            }
            _ => continue, // silent
        };
        if w.write_all(&out).is_err() {
            return;
        }
    }
}

fn connect(f: &Fake) -> Remote {
    Remote::connect(&f.socket).expect("the fake server answers the handshake")
}

fn one_session() -> Listing {
    Listing { sessions: 1, windows: 1, blocks: 1, ..Listing::default() }
}

/// How long `remote.call(method)` took to fail; the test gives up after 30 s.
fn call_fails_in(remote: Remote, method: &'static str, params: Value) -> Duration {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let t = Instant::now();
        let r = remote.call(method, params);
        let _ = tx.send((r, t.elapsed()));
        std::mem::forget(remote); // its threads may still be blocked; nothing to wait for
    });
    let (r, took) = rx.recv_timeout(Duration::from_secs(30)).unwrap_or_else(|_| panic!("{method} hung for 30 s"));
    let e = r.expect_err("no reply came");
    assert_eq!(e.code, "unreachable", "{e:?}");
    took
}

#[test]
fn a_flood_of_lines_that_are_not_the_reply_cant_hold_a_call() {
    let f = serve(one_session());
    let took = call_fails_in(connect(&f), "test.flood", json!({}));
    assert!(took < Duration::from_secs(7), "a call over a flood took {took:?}");
}

#[test]
fn a_line_trickling_in_cant_hold_a_call() {
    let f = serve(one_session());
    let took = call_fails_in(connect(&f), "test.trickle", json!({}));
    assert!(took < Duration::from_secs(7), "a call over a trickle took {took:?}");
}

#[test]
fn a_server_that_stops_reading_cant_hold_a_write() {
    let f = serve(one_session());
    let remote = connect(&f);
    remote.call("test.deafen", json!({})).unwrap();
    let big = "x".repeat(64 << 20);
    let took = call_fails_in(remote, "test.big", json!({ "data": big }));
    assert!(took < Duration::from_secs(7), "a write to a deaf server took {took:?}");
}

#[test]
fn a_spawned_call_to_a_silent_server_ends() {
    let f = serve(one_session()); // which sets the wait to 2 s
    let remote = connect(&f);
    let (tx, rx) = mpsc::channel();
    remote.spawn("test.silent", json!({}), move |r| drop(tx.send(r)));
    let r = rx.recv_timeout(Duration::from_secs(2 + 7)).expect("the spawned call ended");
    assert_eq!(r.expect_err("no reply came").code, "unreachable");
}

#[test]
fn a_line_that_is_not_utf8_doesnt_end_the_link() {
    let events = b"{\"event\":{\"type\":\"x\",\"note\":\"\xff\xfe\"},\"seq\":1}\n".to_vec();
    let f = serve(Listing { events, ..one_session() });
    let remote = connect(&f);
    std::thread::sleep(Duration::from_millis(500));
    assert!(remote.alive(), "one bad line is skipped, not read as a line past the cap");
    assert!(remote.call("server.version", json!({})).is_ok());
}

#[test]
fn a_split_ratio_outside_zero_to_one_is_dropped() {
    let leaf = |b: &str| json!({ "block": { "block_id": b } });
    let split = |ratio: f64| {
        json!({ "split": { "split_id": "split:1", "direction": "horizontal", "ratio": ratio,
            "before": leaf("block:0"), "after": leaf("block:1") } })
    };
    let f = serve(Listing { layout: Some(split(1e39)), ..one_session() });
    let remote = connect(&f);
    assert!(remote.model().sessions[0].windows[0].layout.is_none(), "a ratio of 1e39 gives non-finite rects");
    let f = serve(Listing { layout: Some(split(0.25)), ..one_session() });
    let remote = connect(&f);
    assert!(remote.model().sessions[0].windows[0].layout.is_some(), "a good ratio is kept");
}

#[test]
fn what_a_server_lists_is_capped() {
    let f = serve(Listing { sessions: 1, windows: 40_000, blocks: 40_000, ..Listing::default() });
    let remote = connect(&f);
    let m = remote.model();
    let s = &m.sessions[0];
    assert!(s.windows.len() <= 1024, "{} windows kept", s.windows.len());
    assert!(s.blocks.len() <= 1024, "{} blocks kept", s.blocks.len());
    drop(m);
    let f = serve(Listing { sessions: 300, windows: 1, blocks: 1, ..Listing::default() });
    assert!(connect(&f).model().sessions.len() <= 256);
}

// macOS only: the sidebar rows are the GPUI workspace's.
#[cfg(target_os = "macos")]
#[gpui::test]
fn sidebar_rows_read_each_block_once(cx: &mut gpui::TestAppContext) {
    let f = serve(Listing { sessions: 8, windows: 1024, blocks: 1024, ..Listing::default() });
    let remote = std::rc::Rc::new(connect(&f));
    cx.update(leon_app::view::init);
    let (ws, cx) = cx.add_window_view(move |window, cx| leon_app::view::Workspace::new(remote, window, cx));
    // the best of three: a per-window filter takes far longer every time, a busy machine now and then
    let took = (0..3)
        .map(|_| {
            ws.read_with(cx, |ws, _| {
                let t = Instant::now();
                let (sessions, _) = ws.indicators();
                assert_eq!(sessions.len(), 8);
                t.elapsed()
            })
        })
        .min()
        .unwrap();
    assert!(took < Duration::from_millis(50), "8 sessions of 1024 tabs and 1024 blocks took {took:?}");
}

#[test]
fn a_layouts_leaves_are_capped_and_each_named_once() {
    fn tree(depth: u32, n: &mut u32) -> Value {
        if depth == 0 {
            return json!({ "block": { "block_id": "block:0" } });
        }
        *n += 1;
        let id = format!("split:{n}");
        let (before, after) = (tree(depth - 1, n), tree(depth - 1, n));
        json!({ "split": { "split_id": id, "direction": "horizontal", "ratio": 0.5, "before": before, "after": after } })
    }
    // 32768 leaves, all the one block: the render lays out each, per pane shown
    let f = serve(Listing { layout: Some(tree(15, &mut 0)), ..one_session() });
    let leaves = connect(&f).model().sessions[0].windows[0].layout.as_ref().map_or(0, |t| t.leaves().len());
    assert_eq!(leaves, 0, "a layout naming a block twice is dropped");
}
