//! M3 acceptance test, criterion 11 (.agent/plans/2026-11-14-m3-layout-events.md): a `leon events`
//! reader that stops reading never blocks the server, and is dropped once it falls too far behind.

mod support;

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::process::Stdio;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use support::Env;

/// Two events each (label and view change), ~390 bytes a pair: well past the 8 MiB backlog.
const CHANGES: usize = 30_000;

#[test]
fn c11_a_slow_reader_is_dropped_and_blocks_nothing() {
    let env = Env::new();
    let s = env.ok(&["new"]);
    let mut child = env.cmd().args(["events", "--all", "--json"]).stdout(Stdio::piped()).spawn().unwrap();
    let mut out = BufReader::new(child.stdout.take().unwrap());
    let mut first = String::new();
    out.read_line(&mut first).unwrap();
    assert!(first.contains("snapshot"), "{first}");
    // From here on nobody reads the events process's stdout. This one is read throughout:
    let mut fast = env.events(&["--all"]);

    let started = Instant::now();
    let conn = UnixStream::connect(env.socket()).unwrap();
    conn.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
    let mut replies = BufReader::new(conn.try_clone().unwrap());
    let mut w = conn;
    for i in 0..CHANGES {
        let req = json!({ "id": i, "method": "session.set_label", "params": { "session_id": s, "label": format!("{i:0>100}") } });
        writeln!(w, "{req}").unwrap();
        let mut line = String::new();
        replies.read_line(&mut line).expect("the server stopped answering while a reader was stuck");
        assert!(line.contains("\"result\""), "{line}");
    }
    assert!(started.elapsed() < Duration::from_secs(60), "label changes took {:?}", started.elapsed());

    let (tx, rx) = mpsc::channel();
    let mut status = env.cmd();
    status.args(["--autostart=false", "server", "status"]);
    std::thread::spawn(move || tx.send(status.output().unwrap().status.success()).unwrap());
    assert_eq!(rx.recv_timeout(Duration::from_secs(3)), Ok(true), "server status answered in 3 s");

    for _ in 0..2 * CHANGES {
        fast.next(); // fails on a gap or a 10 s stall
    }
    assert!(fast.child.try_wait().unwrap().is_none(), "the reader that kept up is still connected");

    let (tx, lines) = mpsc::channel();
    std::thread::spawn(move || {
        for line in out.lines() {
            let Ok(line) = line else { break };
            let _ = tx.send(line);
        }
    });
    let end = Instant::now() + Duration::from_secs(10);
    let code = loop {
        if let Some(st) = child.try_wait().unwrap() {
            break st.code();
        }
        assert!(Instant::now() < end, "the stuck reader was never dropped");
        std::thread::sleep(Duration::from_millis(50));
    };
    assert_eq!(code, Some(1), "a dropped stream exits 1");
    let seqs: Vec<u64> = lines
        .iter() // ends at EOF, which the exit above brought
        .map(|l| serde_json::from_str::<Value>(&l).unwrap()["seq"].as_u64().unwrap())
        .collect();
    assert!(!seqs.is_empty() && seqs.len() < 2 * CHANGES, "{} events printed", seqs.len());
    let first_seq: Value = serde_json::from_str(&first).unwrap();
    let start = first_seq["seq"].as_u64().unwrap() + 1;
    for (want, seq) in (start..).zip(seqs) {
        assert_eq!(seq, want, "gapless up to the drop");
    }
}
