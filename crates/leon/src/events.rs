//! `leon events`: attach, print the snapshot, then every event as it comes.

use std::io::Write;

use serde_json::{json, Value};

use crate::client::{Client, Fail};
use crate::commands::pick_session;
use crate::Cli;

pub fn run(c: &mut Client, cli: &Cli, all: bool) -> Result<(), Fail> {
    let (method, params, session) = match (&cli.session, all) {
        (Some(_), false) => {
            let sid = pick_session(c, cli, None)?;
            ("session.attach", json!({ "session_id": sid }), Some(sid))
        }
        (_, true) => ("session.attach_all", json!({ "event_scope": "all" }), None),
        (None, false) => ("session.attach_all", json!({ "event_scope": "session" }), None),
    };
    let snapshot = c.call(method, params)?;
    let mut out = std::io::stdout().lock();
    let first = if cli.json {
        json!({ "snapshot": snapshot, "seq": snapshot["seq"] }).to_string()
    } else {
        format!("attached at seq {}", snapshot["seq"])
    };
    // A closed stdout (`leon events | head`) ends the stream quietly.
    if writeln!(out, "{first}").is_err() {
        return Ok(());
    }
    loop {
        let line = c.next_event()?;
        let text = if cli.json { line.to_string() } else { text(&line) };
        if writeln!(out, "{text}").is_err() {
            return Ok(());
        }
        let e = &line["event"];
        if session.is_some()
            && e["type"] == "session_destroyed"
            && e["payload"]["session_id"].as_str() == session.as_deref()
        {
            return Ok(());
        }
    }
}

/// `seq  session  [block]  name  payload` for people.
fn text(line: &Value) -> String {
    let (seq, e) = (&line["seq"], &line["event"]);
    let s = |v: &Value| v.as_str().unwrap_or("-").to_string();
    if e["type"] != "session_event" {
        let sid = e["payload"]["session_id"].as_str().or(e["payload"]["session"]["session_id"].as_str());
        return format!("{seq}  {}  {}  {}", sid.unwrap_or("-"), s(&e["type"]), e["payload"]);
    }
    let (sid, inner) = (s(&e["payload"]["session_id"]), &e["payload"]["event"]);
    if inner["type"] == "block_event" {
        let b = &inner["payload"];
        return format!("{seq}  {sid}  {}  {}  {}", s(&b["block_id"]), s(&b["name"]), b["payload"]);
    }
    format!("{seq}  {sid}  {}  {}", s(&inner["type"]), inner["payload"])
}
