//! leon in the browser, PR 2 (.agent/plans/2026-11-20-web-full-control.md), criteria 2–3: the key,
//! text and paste events the page sends over its WebSocket reach a program as the exact bytes.

mod support;

use std::time::{Duration, Instant};

use serde_json::{json, Value};
use support::localapi::Serving;
use support::web::{self, Ws};

/// A page's WebSocket on a server with the web page on.
fn page() -> (Serving, Ws) {
    let s = Serving::start("leon.set(\"serve_web\", true)");
    let token = web::token(s.port);
    let ws = Ws::open(s.port, Some(&format!("http://127.0.0.1:{}", s.port)), Some(&token)).expect("let in");
    (s, ws)
}

/// Call `method` and return its result (panics on an error).
fn call(ws: &mut Ws, id: u64, method: &str, params: Value) -> Value {
    ws.send(&json!({"id": id, "method": method, "params": params}).to_string());
    let r = ws.until(method, |v| v["id"] == id);
    assert!(r.get("error").is_none(), "{method}: {r}");
    r["result"].clone()
}

/// A block running `program` (sh -c), watched; waits until its screen says `ready` and the modes
/// it sets are on. Returns (session, block).
fn program(ws: &mut Ws, program: &str, app_cursor: bool, bracketed: bool) -> (String, String) {
    let opts = json!({"command": ["sh", "-c", program]});
    let c = call(ws, 1, "session.create", json!({"layout": {"block": {"options": opts}}}));
    let (s, b) = (c["session_id"].as_str().unwrap().to_string(), c["block_id"].as_str().unwrap().to_string());
    call(ws, 2, "block.watch", json!({"block_ids": [b]}));
    let end = Instant::now() + Duration::from_secs(10);
    let mut seen = String::new();
    loop {
        let m = ws.recv(end.saturating_duration_since(Instant::now())).expect("the program's screen within 10 s");
        let v: Value = serde_json::from_str(&m).unwrap();
        let sc = &v["screen"];
        if sc["block_id"] != b.as_str() {
            continue;
        }
        for row in sc["rows"].as_array().into_iter().flatten() {
            for span in row["spans"].as_array().into_iter().flatten() {
                seen.push_str(span["text"].as_str().unwrap_or(""));
            }
        }
        if seen.contains("ready") && sc["app_cursor"] == app_cursor && sc["bracketed_paste"] == bracketed {
            return (s, b);
        }
    }
}

/// Wait up to 10 s for `path` to hold `n` bytes; returns them.
fn bytes(path: &std::path::Path, n: usize) -> Vec<u8> {
    let end = Instant::now() + Duration::from_secs(10);
    loop {
        let got = std::fs::read(path).unwrap_or_default();
        if got.len() >= n {
            return got;
        }
        assert!(Instant::now() < end, "only {} of {n} bytes arrived: {got:?}", got.len());
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn key(s: &str, b: &str, name: &str) -> Value {
    json!({"session_id": s, "block_id": b, "event": {"type": "key", "key": name}})
}

#[test]
fn keys_and_text_reach_the_program_byte_for_byte() {
    let (env, mut ws) = page();
    // what the page sends for: Enter, Escape, Tab, shift+Tab, Backspace, ArrowUp (application
    // cursor mode is on), ctrl+ArrowLeft, ctrl+c, Alt+b (Option as Meta), Home, PageDown, Delete,
    // F5, ctrl+Space, then typed text through the textarea
    let cases: &[(&str, &[u8])] = &[
        ("enter", b"\r"),
        ("escape", b"\x1b"),
        ("tab", b"\t"),
        ("shift+tab", b"\x1b[Z"),
        ("backspace", b"\x7f"),
        ("up", b"\x1bOA"),
        ("ctrl+left", b"\x1b[1;5D"),
        ("ctrl+c", b"\x03"),
        ("alt+b", b"\x1bb"),
        ("home", b"\x1bOH"),
        ("pagedown", b"\x1b[6~"),
        ("delete", b"\x1b[3~"),
        ("f5", b"\x1b[15~"),
        ("ctrl+space", b"\x00"),
    ];
    let text = "héllo, wörld ✓";
    let mut want: Vec<u8> = cases.iter().flat_map(|(_, b)| b.to_vec()).collect();
    want.extend_from_slice(text.as_bytes());
    let out = env.env.dir.path().join("keys.bin");
    let prog = format!(
        "printf '\\033[?1h'; stty raw -echo; printf ready; head -c {} > '{}'; sleep 30",
        want.len(),
        out.display()
    );
    let (s, b) = program(&mut ws, &prog, true, false);
    for (i, (name, _)) in cases.iter().enumerate() {
        call(&mut ws, 10 + i as u64, "block.event", key(&s, &b, name));
    }
    let write = json!({"session_id": s, "block_id": b, "method": "dev.leon.terminal.write", "args": {"data": text}});
    call(&mut ws, 99, "block.method", write);
    assert_eq!(bytes(&out, want.len()), want, "the bytes the program read");
}

#[test]
fn a_paste_is_bracketed_when_the_program_asks_and_plain_when_not() {
    let (env, mut ws) = page();
    let data = "line one\nline two\r\n\x1btab\there";
    for (bracketed, setup) in [(true, "printf '\\033[?2004h'; "), (false, "")] {
        let want: Vec<u8> = match bracketed {
            true => b"\x1b[200~line one\rline two\rtab\there\x1b[201~".to_vec(),
            false => b"line one\rline two\r\x1btab\there".to_vec(),
        };
        let out = env.env.dir.path().join(format!("paste-{bracketed}.bin"));
        let prog =
            format!("{setup}stty raw -echo; printf ready; head -c {} > '{}'; sleep 30", want.len(), out.display());
        let (s, b) = program(&mut ws, &prog, false, bracketed);
        let paste = json!({"session_id": s, "block_id": b, "event": {"type": "paste", "data": data}});
        call(&mut ws, 50, "block.event", paste);
        assert_eq!(bytes(&out, want.len()), want, "bracketed: {bracketed}");
    }
}
