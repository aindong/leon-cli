//! M22b criterion 8: a connection's palette. `block.watch` and `terminal.lines` resolve a
//! block's colours with the palette the connection sends; without one they are DARK, as before.

mod support;

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::time::{Duration, Instant};

use leon_core::theme::{DARK, LIGHT};
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

/// The colour of the span reading `text` (the last one), in a screen or a `lines` reply.
fn fg(rows: &Value, text: &str) -> Option<u64> {
    let mut spans = rows.as_array()?.iter().flat_map(|r| r["spans"].as_array().cloned().unwrap_or_default());
    spans.rfind(|s| s["text"] == text).and_then(|s| s["fg"].as_u64())
}

fn palette(t: &leon_core::theme::Theme) -> Value {
    json!({ "fg": t.fg, "bg": t.bg, "cursor": t.cursor, "ansi": t.ansi })
}

#[test]
fn a_connection_gets_its_own_palette() {
    let env = Env::new();
    let script = "stty raw; printf '\\112\\033[31m\\121\\033[0m READY\\n'; exec cat >/dev/null";
    let block = env.ok(&["run", "--", "sh", "-c", script]);
    let sid = env.json(&["ls", "--json"])[0]["session_id"].as_str().unwrap().to_string();
    env.screen_until(&block, "the program's output", |s| s.contains("READY"));

    // no palette: DARK, as every client before M22b
    let mut plain = Conn::open(&env);
    let mut early = Vec::new();
    plain.call("block.watch", json!({ "block_ids": [block] }), &mut early);
    let s = early.pop().or_else(|| plain.screen(Duration::from_secs(5))).expect("a screen");
    assert_eq!((fg(&s["rows"], "J"), fg(&s["rows"], "Q")), (Some(DARK.fg as u64), Some(DARK.ansi[1] as u64)));

    // a light palette on another connection
    let mut light = Conn::open(&env);
    let mut early = Vec::new();
    light.call("block.watch", json!({ "block_ids": [block], "palette": palette(&LIGHT) }), &mut early);
    let s = early.pop().or_else(|| light.screen(Duration::from_secs(5))).expect("a screen");
    assert_eq!((fg(&s["rows"], "J"), fg(&s["rows"], "Q")), (Some(LIGHT.fg as u64), Some(LIGHT.ansi[1] as u64)));

    // a new palette on the same watch: the screen again, whole, in it
    let mut harbor = LIGHT;
    harbor.fg = 0x102030;
    harbor.ansi[1] = 0x405060;
    let mut early = Vec::new();
    light.call("block.watch", json!({ "block_ids": [block], "palette": palette(&harbor) }), &mut early);
    let s = early.pop().or_else(|| light.screen(Duration::from_secs(5))).expect("the screen again");
    assert_eq!(s["full"], true, "a palette change sends the whole screen: {s}");
    assert_eq!((fg(&s["rows"], "J"), fg(&s["rows"], "Q")), (Some(0x102030), Some(0x405060)));

    // lines: the palette the call names, else DARK
    let lines = |c: &mut Conn, palette: Option<Value>| {
        let mut args = json!({ "from": 0, "count": 50 });
        if let Some(p) = palette {
            args["palette"] = p;
        }
        let p = json!({ "session_id": sid, "block_id": block, "method": "dev.leon.terminal.lines", "args": args });
        c.call("block.method", p, &mut Vec::new())
    };
    let dark = lines(&mut plain, None);
    let lit = lines(&mut plain, Some(palette(&LIGHT)));
    assert_eq!(fg(&dark["rows"], "Q"), Some(DARK.ansi[1] as u64), "{dark}");
    assert_eq!(fg(&lit["rows"], "Q"), Some(LIGHT.ansi[1] as u64), "{lit}");
}
