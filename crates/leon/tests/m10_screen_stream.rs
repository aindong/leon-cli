//! M10 criteria 6 and 7 (.agent/plans/2026-11-14-m10-app-attaches.md): the screen transport. A
//! connection that watches a block gets a full screen, then only changed rows; a connection that
//! stops reading is never dropped and never queues more than one screen per block.

mod support;

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use support::Env;

/// One NDJSON connection; every read has a deadline.
struct Conn {
    r: BufReader<UnixStream>,
    id: u64,
}

impl Conn {
    fn open(env: &Env) -> Conn {
        let s = UnixStream::connect(env.socket()).unwrap();
        Conn { r: BufReader::new(s), id: 0 }
    }

    fn send(&mut self, method: &str, params: Value) -> u64 {
        self.id += 1;
        let mut line = serde_json::to_vec(&json!({ "id": self.id, "method": method, "params": params })).unwrap();
        line.push(b'\n');
        self.r.get_mut().write_all(&line).unwrap();
        self.id
    }

    /// The next line within `wait`, or None.
    fn line(&mut self, wait: Duration) -> Option<Value> {
        self.r.get_ref().set_read_timeout(Some(wait)).unwrap();
        let mut s = String::new();
        match self.r.read_line(&mut s) {
            Ok(0) => panic!("the server closed the connection"),
            Ok(_) => Some(serde_json::from_str(&s).unwrap_or_else(|e| panic!("not JSON ({e}): {s}"))),
            Err(_) => None,
        }
    }

    /// Call and wait for the reply, keeping screen lines that arrive first in `screens`.
    fn call(&mut self, method: &str, params: Value, screens: &mut Vec<Value>) -> Value {
        let id = self.send(method, params);
        let end = Instant::now() + Duration::from_secs(10);
        while Instant::now() < end {
            let Some(v) = self.line(Duration::from_secs(10)) else { continue };
            if v.get("screen").is_some() {
                screens.push(v["screen"].clone());
            } else if v["id"] == id {
                assert!(v["error"].is_null(), "{method}: {}", v["error"]);
                return v["result"].clone();
            }
        }
        panic!("no reply to {method}");
    }

    /// The next screen line within `wait`.
    fn screen(&mut self, wait: Duration) -> Option<Value> {
        let end = Instant::now() + wait;
        while Instant::now() < end {
            if let Some(v) = self.line(end.saturating_duration_since(Instant::now())) {
                if v.get("screen").is_some() {
                    return Some(v["screen"].clone());
                }
            }
        }
        None
    }
}

/// A row's text: its spans placed at their columns.
fn row_text(row: &Value) -> String {
    let mut out = String::new();
    for s in row["spans"].as_array().unwrap() {
        let col = s["col"].as_u64().unwrap() as usize;
        while out.chars().count() < col {
            out.push(' ');
        }
        out.push_str(s["text"].as_str().unwrap());
    }
    out
}

fn screen_has(screen: &Value, text: &str) -> bool {
    screen["rows"].as_array().unwrap().iter().any(|r| row_text(r).contains(text))
}

#[test]
fn watch_sends_a_full_screen_then_only_changed_rows() {
    let env = Env::new();
    let script = "stty raw echo; printf '\\033[31mRED\\033[0m READY\\n'; exec cat >/dev/null";
    let block = env.ok(&["run", "--", "sh", "-c", script]);
    let sid = env.json(&["ls", "--json"])[0]["session_id"].as_str().unwrap().to_string();
    env.screen_until(&block, "the program's prompt", |s| s.contains("READY"));

    let mut c = Conn::open(&env);
    let mut early = Vec::new();
    c.call("block.watch", json!({ "block_ids": [block] }), &mut early);
    let full = early.pop().or_else(|| c.screen(Duration::from_secs(5))).expect("a first screen");
    assert_eq!(full["block_id"], block.as_str());
    assert_eq!(full["full"], true, "the first screen is whole: {full}");
    let lines = full["lines"].as_u64().unwrap() as usize;
    assert_eq!(full["rows"].as_array().unwrap().len(), lines, "every row: {full}");
    assert!(full["cols"].as_u64().unwrap() >= 2);
    assert!(screen_has(&full, "RED READY"), "{full}");
    let red = full["rows"].as_array().unwrap().iter().flat_map(|r| r["spans"].as_array().unwrap().clone());
    let red: Vec<Value> = red.filter(|s| s["text"].as_str().unwrap().starts_with("RED")).collect();
    assert!(!red.is_empty() && red[0]["fg"].as_u64().is_some(), "colours are RGB numbers: {full}");
    assert_ne!(red[0]["fg"], json!(leon_app::theme::DARK.fg), "red text isn't the default colour: {}", red[0]);
    assert!(full["cursor"].is_object(), "the cursor: {full}");
    assert!(full["app_cursor"].is_boolean() && full["bracketed_paste"].is_boolean(), "the modes: {full}");

    // the same shape as a snapshot method
    let snap = c.call(
        "block.method",
        json!({ "session_id": sid, "block_id": block, "method": "dev.leon.terminal.screen" }),
        &mut early,
    );
    assert_eq!(snap["full"], true);
    assert_eq!(snap["rows"].as_array().unwrap().len(), lines);
    assert!(screen_has(&snap, "RED READY"));

    // one key: only the rows it changed
    let write =
        json!({ "session_id": sid, "block_id": block, "method": "dev.leon.terminal.write", "args": { "data": "Q" } });
    c.call("block.method", write, &mut early);
    let mut got = early.into_iter().filter(|s| s["full"] == false).collect::<Vec<_>>();
    let end = Instant::now() + Duration::from_secs(5);
    while !got.iter().any(|s| screen_has(s, "Q")) && Instant::now() < end {
        got.extend(c.screen(Duration::from_millis(500)));
    }
    let update = got.iter().find(|s| screen_has(s, "Q")).unwrap_or_else(|| panic!("no update with the echo: {got:?}"));
    assert_eq!(update["full"], false, "{update}");
    let rows = update["rows"].as_array().unwrap();
    assert!(!rows.is_empty() && rows.len() < lines, "only changed rows: {update}");
    assert!(rows.iter().all(|r| r["line"].as_u64().is_some()), "each row says where it goes: {update}");

    // an empty set stops the pushes
    let mut late = Vec::new();
    c.call("block.watch", json!({ "block_ids": [] }), &mut late);
    let write =
        json!({ "session_id": sid, "block_id": block, "method": "dev.leon.terminal.write", "args": { "data": "Z" } });
    c.call("block.method", write, &mut late);
    env.screen_until(&block, "the second echo", |s| s.contains('Z'));
    assert!(c.screen(Duration::from_millis(300)).is_none(), "no screen after unwatching");

    let help = env.json(&["api", "list", "--json"]);
    assert!(help.to_string().contains("block.watch"), "help.all lists block.watch");
}

#[test]
fn a_reader_that_stops_gets_one_current_screen_not_a_backlog() {
    let env = Env::new();
    let block = env.ok(&["run", "--", "sh", "-c", "read go; seq 1 300000; echo FLOOD-END; exec sleep 300"]);
    let sid = env.json(&["ls", "--json"])[0]["session_id"].as_str().unwrap().to_string();
    let mut c = Conn::open(&env);
    let mut early = Vec::new();
    c.call("block.watch", json!({ "block_ids": [block] }), &mut early);
    assert!(early.pop().or_else(|| c.screen(Duration::from_secs(5))).is_some(), "a first screen");
    let go =
        json!({ "session_id": sid, "block_id": block, "method": "dev.leon.terminal.write", "args": { "data": "\r" } });
    c.call("block.method", go, &mut early);

    std::thread::sleep(Duration::from_secs(2)); // read nothing while the flood runs
    env.screen_until(&block, "the flood's end", |s| s.contains("FLOOD-END"));

    let mut got = early;
    while let Some(s) = c.screen(Duration::from_millis(500)) {
        got.push(s);
        assert!(got.len() < 100, "a backlog of screens built up: {} lines", got.len());
    }
    let last = got.last().expect("screens after the pause");
    // the client patches rows into its copy; the newest rows must show the end
    assert!(got.iter().rev().take(3).any(|s| screen_has(s, "FLOOD-END")), "the last screens show the end: {last}");
    assert!(env.ok(&["--json", "server", "status"]).contains("running"), "the connection was not the server's end");
}
