//! `leon events`: attach, print the snapshot, then every event as it comes. Over a link (-H) the
//! attach is resumable: when the link drops, it reconnects and resumes after the last seq printed,
//! or, if the server can't resume (it restarted), prints a fresh snapshot.

use std::io::Write;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use crate::client::{Client, Fail, Server};
use crate::commands::pick_session;
use crate::Cli;

/// How long a dropped link is retried: as long as the server keeps a dropped stream.
const GIVE_UP: Duration = leon_server::resume::HOLD;

pub fn run(mut c: Client, cli: &Cli, all: bool, server: &Server) -> Result<(), Fail> {
    let (method, mut params, session) = match (&cli.session, all) {
        (Some(_), false) => {
            let sid = pick_session(&mut c, cli, None)?;
            ("session.attach", json!({ "session_id": sid }), Some(sid))
        }
        (_, true) => ("session.attach_all", json!({ "event_scope": "all" }), None),
        (None, false) => ("session.attach_all", json!({ "event_scope": "session" }), None),
    };
    if server.link.is_some() {
        params["resumable"] = json!(true);
    }
    let snapshot = c.call(method, params.clone())?;
    let mut resume = snapshot["resume"].as_str().map(String::from);
    let mut seq = snapshot["seq"].as_u64().unwrap_or(0);
    let mut out = std::io::stdout().lock();
    let first = if cli.json {
        json!({ "snapshot": snapshot, "seq": snapshot["seq"] }).to_string()
    } else {
        format!("attached at seq {seq}")
    };
    // A closed stdout (`leon events | head`) ends the stream quietly.
    if writeln!(out, "{first}").is_err() {
        return Ok(());
    }
    loop {
        let line = match (c.next_event(), &resume) {
            (Ok(line), _) => line,
            (Err(f), None) => return Err(f),
            (Err(_), Some(token)) => {
                let (again, fresh) = reconnect(server, token, seq, method, &params)?;
                c = again;
                let Some(snapshot) = fresh else { continue };
                resume = snapshot["resume"].as_str().map(String::from);
                seq = snapshot["seq"].as_u64().unwrap_or(0);
                let text = match cli.json {
                    true => json!({ "snapshot": snapshot, "seq": seq }).to_string(),
                    false => format!("re-attached at seq {seq}; events while the link was down are not shown"),
                };
                if writeln!(out, "{text}").is_err() {
                    return Ok(());
                }
                continue;
            }
        };
        seq = line["seq"].as_u64().unwrap_or(seq);
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

/// Get a dropped link back (backing off from 100 ms to 5 s) and resume after `seq`; when the
/// server refuses the resume, attach again and return the new snapshot.
fn reconnect(
    server: &Server,
    token: &str,
    seq: u64,
    method: &str,
    params: &Value,
) -> Result<(Client, Option<Value>), Fail> {
    let end = Instant::now() + GIVE_UP;
    let mut wait = Duration::from_millis(100);
    loop {
        match server.connect(false) {
            Ok(mut c) => match c.call("session.resume", json!({ "resume": token, "seq": seq })) {
                Ok(_) => return Ok((c, None)),
                Err(Fail::Api(e)) if e.code == "resume_failed" => {
                    let snapshot = c.call(method, params.clone())?;
                    return Ok((c, Some(snapshot)));
                }
                Err(Fail::Unreachable(_)) => {}
                Err(f) => return Err(f),
            },
            Err(Fail::Unreachable(_)) => {}
            Err(f) => return Err(f),
        }
        if Instant::now() + wait > end {
            let name = server.link.as_ref().map_or("the host", |(n, _)| n.as_str());
            return Err(Fail::Unreachable(format!("lost the link to {name} for {} s", GIVE_UP.as_secs())));
        }
        std::thread::sleep(wait);
        wait = (wait * 2).min(Duration::from_secs(5));
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
