//! Impact proofs for M10's screen transport: what it must not change outside itself. Screen lines
//! reach only connections that asked for them (`leon events` and scripts see none), watching a
//! block never keeps its program alive after it closes, and no client can grow a grid without bound.

mod support;

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use support::Env;

fn send(s: &mut UnixStream, id: u64, method: &str, params: Value) {
    let mut line = serde_json::to_vec(&json!({ "id": id, "method": method, "params": params })).unwrap();
    line.push(b'\n');
    s.write_all(&line).unwrap();
}

/// Every line `r` gets within `wait`.
fn lines(r: &mut BufReader<UnixStream>, wait: Duration) -> Vec<Value> {
    let end = Instant::now() + wait;
    let mut out = Vec::new();
    while Instant::now() < end {
        r.get_ref()
            .set_read_timeout(Some(end.saturating_duration_since(Instant::now()).max(Duration::from_millis(1))))
            .unwrap();
        let mut s = String::new();
        match r.read_line(&mut s) {
            Ok(n) if n > 0 => out.push(serde_json::from_str(&s).unwrap()),
            _ => break,
        }
    }
    out
}

#[test]
fn screens_go_only_to_watchers_and_never_keep_a_program_alive() {
    let env = Env::new();
    let block = env.ok(&["run", "--", "sh", "-c", "while :; do echo out; sleep 0.05; done"]);
    let sid = env.json(&["ls", "--json"])[0]["session_id"].as_str().unwrap().to_string();
    let pid = env.json(&["block", "inspect", &block, "--json"])["pid"].as_i64().unwrap() as i32;

    let mut events = UnixStream::connect(env.socket()).unwrap();
    send(&mut events, 1, "session.attach_all", json!({ "event_scope": "all" }));
    let mut watcher = UnixStream::connect(env.socket()).unwrap();
    send(&mut watcher, 1, "block.watch", json!({ "block_ids": [block] }));
    let (mut er, mut wr) = (BufReader::new(events), BufReader::new(watcher));
    let watched = lines(&mut wr, Duration::from_millis(500));
    assert!(watched.iter().any(|l| l.get("screen").is_some()), "the watcher gets screens");

    env.ok(&["block", "close", &block]);
    let seen = lines(&mut er, Duration::from_millis(500));
    assert!(!seen.is_empty(), "the events connection got its reply and events");
    for l in &seen {
        assert!(l.get("screen").is_none(), "a connection that didn't watch got a screen: {l}");
        assert!(l.get("id").is_some() || l["seq"].as_u64().is_some(), "every event line keeps its seq: {l}");
    }
    // the watcher still holds its watch, and the program still ends
    let end = Instant::now() + Duration::from_secs(10);
    // SAFETY: signal 0 only asks whether the pid exists.
    while unsafe { libc::kill(pid, 0) } == 0 {
        assert!(Instant::now() < end, "the closed block's program kept running while watched");
        std::thread::sleep(Duration::from_millis(50));
    }
    let _ = sid;
}

#[test]
fn a_client_cannot_grow_a_grid_without_bound() {
    let env = Env::new();
    let block = env.ok(&["run", "--", "sleep", "300"]);
    let sid = env.json(&["ls", "--json"])[0]["session_id"].as_str().unwrap().to_string();
    let call = |args: Value| {
        let p = json!({ "session_id": sid, "block_id": block, "method": "dev.leon.terminal.resize", "args": args });
        env.json(&["api", "call", "block.method", &p.to_string()])
    };
    let size = call(json!({ "rows": 65535, "cols": 65535 }));
    assert!(size["rows"].as_u64().unwrap() <= 500 && size["cols"].as_u64().unwrap() <= 1000, "capped: {size}");
    let size = call(json!({ "rows": 0, "cols": 0 }));
    assert_eq!((size["rows"].as_u64(), size["cols"].as_u64()), (Some(1), Some(2)), "and floored: {size}");
}
