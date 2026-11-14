//! M16a criterion 8 (.agent/plans/2026-11-15-m16a-remote-ssh.md): `session.resume` over a plain
//! socket. Lines the server wrote but the client never read are replayed once; the token works
//! for every later drop; a takeover ends the earlier connection; bad resumes fail.

mod support;

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::time::Duration;

use serde_json::{json, Value};
use support::Env;

struct Conn {
    r: BufReader<UnixStream>,
    id: u64,
}

impl Conn {
    fn open(env: &Env) -> Conn {
        let s = UnixStream::connect(env.socket()).unwrap();
        s.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
        Conn { r: BufReader::new(s), id: 0 }
    }

    fn line(&mut self) -> Option<Value> {
        let mut l = String::new();
        match self.r.read_line(&mut l) {
            Ok(0) | Err(_) => None,
            Ok(_) => Some(serde_json::from_str(&l).unwrap()),
        }
    }

    /// The reply to a call (events that come first are kept in `seen`).
    fn call(&mut self, method: &str, params: Value, seen: &mut Vec<Value>) -> Value {
        self.id += 1;
        let req = json!({ "id": self.id, "method": method, "params": params });
        writeln!(self.r.get_mut(), "{req}").unwrap();
        loop {
            let v = self.line().expect("a reply");
            if v.get("event").is_some() {
                seen.push(v);
            } else {
                return v;
            }
        }
    }

    /// Events until a `session_created` labelled `label`.
    fn until_created(&mut self, label: &str, seen: &mut Vec<Value>) {
        loop {
            let v = self.line().unwrap_or_else(|| panic!("closed before {label} was created"));
            let done = v["event"]["type"] == "session_created" && v["event"]["payload"]["session"]["label"] == label;
            seen.push(v);
            if done {
                return;
            }
        }
    }
}

fn seqs(events: &[Value]) -> Vec<u64> {
    events.iter().map(|e| e["seq"].as_u64().unwrap()).collect()
}

fn created(events: &[Value]) -> Vec<String> {
    events
        .iter()
        .filter(|e| e["event"]["type"] == "session_created")
        .map(|e| e["event"]["payload"]["session"]["label"].as_str().unwrap().to_string())
        .collect()
}

#[test]
fn resume_replays_unread_lines_once_and_the_token_lasts() {
    let env = Env::new();
    env.ok(&["ls"]); // start the server
    let mut none = vec![];
    let mut c1 = Conn::open(&env);
    let plain = c1.call("session.attach_all", json!({ "event_scope": "all" }), &mut none);
    assert!(plain["result"].get("resume").is_none(), "only resumable attaches get a token: {plain}");
    drop(c1);

    let mut c1 = Conn::open(&env);
    let r = c1.call("session.attach_all", json!({ "event_scope": "all", "resumable": true }), &mut none);
    let token = r["result"]["resume"].as_str().unwrap_or_else(|| panic!("no resume token: {r}")).to_string();
    let s = r["result"]["seq"].as_u64().unwrap();
    env.ok(&["new", "a"]);
    env.ok(&["new", "b"]);
    // written to c1's socket, never read: they must come again on resume
    std::thread::sleep(Duration::from_millis(300));
    drop(c1);
    env.ok(&["new", "c"]);

    let mut got = vec![];
    let mut c2 = Conn::open(&env);
    let r = c2.call("session.resume", json!({ "resume": token, "seq": s }), &mut got);
    assert!(r.get("error").is_none(), "resume failed: {r}");
    c2.until_created("c", &mut got);
    assert_eq!(seqs(&got), (s + 1..s + 1 + got.len() as u64).collect::<Vec<_>>(), "gapless from S+1, once each");
    assert_eq!(created(&got), ["a", "b", "c"]);
    let last = *seqs(&got).last().unwrap();

    // the same token after a second drop
    drop(c2);
    env.ok(&["new", "d"]);
    let mut again = vec![];
    let mut c3 = Conn::open(&env);
    let r = c3.call("session.resume", json!({ "resume": token, "seq": last }), &mut again);
    assert!(r.get("error").is_none(), "second resume failed: {r}");
    c3.until_created("d", &mut again);
    assert_eq!(seqs(&again)[0], last + 1);
    let last = *seqs(&again).last().unwrap();

    // a takeover while c3 is open ends c3
    let mut c4 = Conn::open(&env);
    let r = c4.call("session.resume", json!({ "resume": token, "seq": last }), &mut none);
    assert!(r.get("error").is_none(), "takeover failed: {r}");
    // the server closes c3: an end of stream, not a read timing out
    let mut rest = vec![];
    loop {
        let mut l = String::new();
        match c3.r.read_line(&mut l) {
            Ok(0) => break,
            Ok(_) => rest.push(serde_json::from_str::<Value>(&l).unwrap()),
            Err(e) => panic!("c3 was never closed: {e}"),
        }
    }
    assert!(rest.iter().all(|v| v.get("event").is_some()), "c3 only gets events, then closes: {rest:?}");
    env.ok(&["new", "e"]);
    let mut tail = vec![];
    c4.until_created("e", &mut tail);
    assert_eq!(seqs(&tail)[0], last + 1);

    let fails = |c: &mut Conn, params: Value| {
        let r = c.call("session.resume", params, &mut vec![]);
        assert_eq!(r["error"]["code"], "resume_failed", "{r}");
    };
    let mut c5 = Conn::open(&env);
    fails(&mut c5, json!({ "resume": "no-such-token", "seq": 0 }));
    fails(&mut c5, json!({ "resume": token, "seq": 1_000_000 }));
    let mut c6 = Conn::open(&env);
    c6.call("session.attach_all", json!({ "event_scope": "session" }), &mut none);
    fails(&mut c6, json!({ "resume": token, "seq": last }));
}

#[test]
fn a_seq_older_than_the_kept_lines_fails() {
    let env = Env::new();
    let sid = env.ok(&["new", "big"]);
    let mut none = vec![];
    let mut c1 = Conn::open(&env);
    let r = c1.call("session.attach_all", json!({ "event_scope": "all", "resumable": true }), &mut none);
    let token = r["result"]["resume"].as_str().unwrap().to_string();
    let s = r["result"]["seq"].as_u64().unwrap();
    // labels are at most 128 characters: many renames, sent in one go on another connection
    let mut other = Conn::open(&env);
    let n = 4000;
    let mut batch = String::new();
    for i in 1..=n {
        let label = format!("{i:0>120}");
        let req = json!({ "id": i, "method": "session.set_label", "params": { "session_id": sid, "label": label } });
        batch.push_str(&format!("{req}\n"));
    }
    // written from a thread: the server stops reading while our replies fill its socket
    let mut w = other.r.get_ref().try_clone().unwrap();
    let writer = std::thread::spawn(move || w.write_all(batch.as_bytes()).unwrap());
    for _ in 0..n {
        other.line().expect("a rename's reply");
    }
    writer.join().unwrap();
    // c1 reads every one of them, so they are all written and the kept lines roll over
    let last = format!("{n:0>120}");
    let mut written = 0;
    loop {
        let v = c1.line().expect("c1's events");
        written += v.to_string().len();
        if v["event"]["payload"]["event"]["payload"]["label"] == last.as_str() {
            break;
        }
    }
    assert!(written > 1 << 20, "more than the kept 1 MiB went out: {written}");
    drop(c1);
    let mut c2 = Conn::open(&env);
    let r = c2.call("session.resume", json!({ "resume": token, "seq": s }), &mut none);
    assert_eq!(r["error"]["code"], "resume_failed", "over 1 MiB went out since S: {r}");
}
