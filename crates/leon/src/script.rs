//! `leon do`, `leon actions`, `leon keymap`, `leon config reload`.
//!
//! An action runs on the server (`client.action`). A script runs here, in this process, on its own
//! socket connection: a reader thread hands every line to a channel, so the script's calls and its
//! event stream share the connection and `leon.wait`/timers are a `recv_timeout`. A script can loop
//! or sleep for ever without the server noticing.

use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use serde_json::{json, Map, Value};

use crate::blocks::{block_query, pick_block};
use crate::client::{Client, Fail};
use crate::commands::{print_json, read_args, resolve};
use crate::{Cli, DoArgs};

/// Event and reply lines longer than this end the script, like the CLI client's limit.
const MAX_LINE: u64 = 64 << 20;

/// A failed call is a failed command (exit 1), even when the error is a missing target.
fn failed(f: Fail) -> Fail {
    match f {
        Fail::Api(e) => Fail::Other(e.message),
        f => f,
    }
}

pub fn run(c: &mut Client, cli: &Cli, d: &DoArgs, socket: &Path) -> Result<(), Fail> {
    let (target, words) = match (&d.code, &d.action) {
        (None, None) => match d.words.split_first() {
            Some((t, rest)) => (Some(t.as_str()), rest),
            None => return Err(Fail::Other("leon do: name an ACTION or SCRIPT, or pass -e CODE".into())),
        },
        _ => (None, &d.words[..]),
    };
    let args = args(d.args.as_deref(), words)?;
    let (session_id, block_id) = here(c, cli)?;
    let script = target.filter(|t| t.ends_with(".lua") || t.contains('/') || Path::new(t).is_file());
    if d.code.is_none() && script.is_none() {
        let action = d.action.as_deref().or(target).unwrap_or_default();
        let origin = d.origin.as_deref().unwrap_or("cli");
        let params = json!({ "action": action, "args": args, "origin": origin,
                             "session_id": session_id, "block_id": block_id });
        let r = c.call("client.action", params).map_err(failed)?;
        match &r["result"] {
            Value::Null => {}
            Value::String(s) => println!("{s}"),
            v => print_json(v),
        }
        return Ok(());
    }
    let (src, name, dir) = match (&d.code, script) {
        (Some(code), _) => (code.clone().into_bytes(), "=(command line)".to_string(), PathBuf::from(".")),
        (None, Some(path)) => {
            let src = std::fs::read(path).map_err(|e| Fail::Other(format!("{path}: {e}")))?;
            let dir = Path::new(path).parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
            (src, format!("@{path}"), dir.to_path_buf())
        }
        (None, None) => unreachable!("handled as an action above"),
    };
    let stream = UnixStream::connect(socket).map_err(|e| Fail::Unreachable(format!("can't reach the server: {e}")))?;
    let mut host = Socket::new(stream)?;
    // Attach before the script runs, so it misses nothing from its first line on.
    if d.all {
        leon_lua::Host::call(&mut host, "session.attach_all", json!({ "event_scope": "all" }), None)
            .map_err(Fail::Other)?;
    } else if let Some(sid) = &session_id {
        leon_lua::Host::call(&mut host, "session.attach", json!({ "session_id": sid }), None).map_err(Fail::Other)?;
    }
    let opts =
        leon_lua::ScriptOptions { name, dir, args, session_id, block_id, run_for: d.run_for, fail_fast: d.fail_fast };
    match leon_lua::run_script(&src, Box::new(host), opts) {
        Ok(0) => Ok(()),
        Ok(code) => Err(Fail::Exit(code)),
        Err(msg) => Err(Fail::Other(msg)),
    }
}

/// `--args` (an object) with `k=v` words on top; a value is JSON if it parses, else a string.
fn args(json: Option<&str>, words: &[String]) -> Result<Value, Fail> {
    let mut obj = match json {
        None => Map::new(),
        Some(_) => match read_args(json)? {
            Value::Object(m) => m,
            v => return Err(Fail::Other(format!("--args must be a JSON object, got {v}"))),
        },
    };
    for w in words {
        let (k, v) = w.split_once('=').ok_or_else(|| Fail::Other(format!("expected k=v, got {w:?}")))?;
        obj.insert(k.into(), serde_json::from_str(v).unwrap_or_else(|_| Value::String(v.into())));
    }
    Ok(Value::Object(obj))
}

/// The caller's session and block: `-b`/`$LEON_BLOCK` (and its session), else `-s`/`$LEON_SESSION`,
/// else the only session; either may be none.
fn here(c: &mut Client, cli: &Cli) -> Result<(Option<String>, Option<String>), Fail> {
    if block_query(cli, None).is_some() {
        let (sid, bid) = pick_block(c, cli, None)?;
        return Ok((Some(sid), Some(bid)));
    }
    let env = std::env::var("LEON_SESSION").ok().filter(|s| !s.is_empty());
    if let Some(q) = cli.session.as_deref().or(env.as_deref()) {
        return Ok((Some(resolve(c, "session", q, None)?), None));
    }
    let r = c.call("session.list", json!({}))?;
    Ok(match r["sessions"].as_array().map(Vec::as_slice) {
        Some([only]) => (only["session_id"].as_str().map(str::to_string), None),
        _ => (None, None),
    })
}

/// The script's connection to the server.
struct Socket {
    write: UnixStream,
    lines: mpsc::Receiver<Result<Value, String>>,
    /// Events that arrived while waiting for a reply.
    events: VecDeque<Value>,
    next_id: u64,
}

impl Socket {
    fn new(stream: UnixStream) -> Result<Self, Fail> {
        let read = stream.try_clone().map_err(|e| Fail::Other(e.to_string()))?;
        let (tx, lines) = mpsc::channel();
        std::thread::spawn(move || {
            let mut r = BufReader::new(read);
            loop {
                let mut line = Vec::new();
                let line = match (&mut r).take(MAX_LINE + 1).read_until(b'\n', &mut line) {
                    Ok(0) | Err(_) => return,
                    Ok(_) if line.len() as u64 > MAX_LINE => Err("a server line is too long".to_string()),
                    Ok(_) => serde_json::from_slice(&line).map_err(|e| format!("bad server line: {e}")),
                };
                let stop = line.is_err();
                if tx.send(line).is_err() || stop {
                    return;
                }
            }
        });
        Ok(Socket { write: stream, lines, events: VecDeque::new(), next_id: 1 })
    }

    fn recv(&self, timeout: Option<Duration>) -> Result<Option<Value>, String> {
        let gone = || "lost the server connection".to_string();
        match timeout {
            None => self.lines.recv().map_err(|_| gone())?.map(Some),
            Some(t) => match self.lines.recv_timeout(t) {
                Ok(line) => line.map(Some),
                Err(mpsc::RecvTimeoutError::Timeout) => Ok(None),
                Err(mpsc::RecvTimeoutError::Disconnected) => Err(gone()),
            },
        }
    }
}

impl leon_lua::Host for Socket {
    fn call(&mut self, method: &str, params: Value, _: Option<Instant>) -> Result<Value, String> {
        let id = self.next_id;
        self.next_id += 1;
        let mut line = serde_json::to_vec(&json!({ "id": id, "method": method, "params": params })).expect("json");
        line.push(b'\n');
        self.write.write_all(&line).map_err(|e| format!("lost the server connection: {e}"))?;
        loop {
            let mut v = self.recv(None)?.expect("no timeout");
            if v.get("event").is_some() {
                self.events.push_back(v);
            } else if v["id"] == id {
                return match v.get("error").filter(|e| !e.is_null()) {
                    Some(e) => Err(format!(
                        "{}: {}",
                        e["code"].as_str().unwrap_or("error"),
                        e["message"].as_str().unwrap_or("")
                    )),
                    None => Ok(v["result"].take()),
                };
            }
        }
    }

    fn next_event(&mut self, timeout: Option<Duration>) -> Result<Option<Value>, String> {
        if let Some(e) = self.events.pop_front() {
            return Ok(Some(e));
        }
        let end = timeout.map(|t| Instant::now() + t);
        loop {
            let left = end.map(|e| e.saturating_duration_since(Instant::now()));
            match self.recv(left)? {
                None => return Ok(None),
                Some(v) if v.get("event").is_some() => return Ok(Some(v)),
                Some(_) => {} // a stray reply: nothing else is in flight
            }
        }
    }
}

pub fn reload(c: &mut Client, cli: &Cli) -> Result<(), Fail> {
    let r = c.call("server.config_reload", json!({})).map_err(failed)?;
    if cli.json {
        print_json(&r);
        return Ok(());
    }
    let n = |k: &str| r[k].as_u64().unwrap_or(0);
    println!(
        "{}: {} bindings, {} modes, {} actions (generation {})",
        r["path"].as_str().unwrap_or(""),
        n("bindings"),
        n("modes"),
        n("actions"),
        n("generation")
    );
    for line in r["diagnostics"].as_array().into_iter().flatten().chain(r["logs"].as_array().into_iter().flatten()) {
        println!("{}", line.as_str().unwrap_or(""));
    }
    Ok(())
}

pub fn actions(c: &mut Client, cli: &Cli) -> Result<(), Fail> {
    let km = c.call("client.keymap", json!({}))?;
    let actions = km["server_actions"].as_array().cloned().unwrap_or_default();
    if cli.json {
        print_json(&Value::Array(actions));
        return Ok(());
    }
    for a in &actions {
        let s = |k: &str| a[k].as_str().unwrap_or("").to_string();
        println!("{:<24} {:<28} {}", s("name"), s("title"), s("source"));
    }
    Ok(())
}

pub fn keymap(c: &mut Client, cli: &Cli) -> Result<(), Fail> {
    let km = c.call("client.keymap", json!({}))?;
    if cli.json {
        print_json(&km);
        return Ok(());
    }
    println!("generation {}  {}", km["generation"], km["path"].as_str().unwrap_or("(no config)"));
    for b in km["bindings"].as_array().into_iter().flatten() {
        let key = match b["mode"].as_str() {
            Some(m) => format!("{m}/{}", b["key"].as_str().unwrap_or("")),
            None => b["key"].as_str().unwrap_or("").to_string(),
        };
        let target = match (b["action"].as_str(), &b["args"]) {
            (Some(a), Value::Null) => a.to_string(),
            (Some(a), args) => format!("{a} {args}"),
            (None, _) => "fn".to_string(),
        };
        println!("{key:<24} {target:<28} {}", b["source"].as_str().unwrap_or(""));
    }
    for m in km["modes"].as_array().into_iter().flatten() {
        let ex = if m["exclusive"] == true { " (exclusive)" } else { "" };
        println!("mode {}{ex}  {}", m["name"].as_str().unwrap_or(""), m["source"].as_str().unwrap_or(""));
    }
    Ok(())
}
