//! Terminal commands: run, send, send-key, capture, wait, and `block ls|inspect|close`.

use std::io::{IsTerminal, Write};
use std::time::Duration;

use leon_core::terminal::NAMESPACE as TERMINAL;
use leon_core::ApiError;
use serde_json::{json, Value};

use crate::client::{Client, Fail};
use crate::commands::{json_out, label, own_session, pick_session, print_json, read_args, resolve};
use crate::{BlockCmd, Cli, Cmd};
use clap::{Args, ValueEnum};

#[derive(Args)]
pub struct NotifyArgs {
    /// The state to report.
    #[arg(long, value_enum, default_value = "done")]
    state: NotifyState,
    /// What a blocked program waits for: permission, question or auth (with --state blocked).
    #[arg(long, value_enum)]
    kind: Option<NotifyKind>,
    /// One line naming what it is about (at most 192 bytes).
    #[arg(long, value_name = "T")]
    title: Option<String>,
    /// One line saying more (at most 2048 bytes).
    #[arg(long, value_name = "B")]
    body: Option<String>,
}

#[derive(Clone, Copy, ValueEnum)]
pub enum NotifyKind {
    Permission,
    Question,
    Auth,
}

#[derive(Clone, Copy, ValueEnum)]
pub enum NotifyState {
    Working,
    Blocked,
    Done,
    Error,
}

pub fn run(c: &mut Client, cli: &Cli) -> Result<(), Fail> {
    match cli.cmd() {
        Cmd::Run { label, keep_open, wait, output, cwd, command } => {
            let cwd = crate::commands::block_cwd(cli, cwd.as_deref())?;
            let print = *wait && output.unwrap_or_else(|| !std::io::stdout().is_terminal());
            // the block's text goes with it, so it stays open until it has been read
            let on_completion = !keep_open && !print;
            let options = json!({ "command": command, "cwd": cwd, "exit": { "on_completion": on_completion } });
            let layout = json!({ "block": { "label": label, "options": options } });
            let (sid, r) = if no_session_named(c, cli)? {
                let r = c.call("session.create", json!({ "layout": layout }))?;
                (r["session_id"].as_str().unwrap_or_default().to_string(), r)
            } else {
                let sid = pick_session(c, cli, None)?;
                let r = c.call("session.new_window", json!({ "session_id": sid, "layout": layout }))?;
                (sid, r)
            };
            let block_id = r["block_id"].as_str().unwrap_or_default().to_string();
            println!("{block_id}");
            if *wait {
                let status = wait_for(c, &block_id, None);
                if print {
                    // a block closed meanwhile (by hand, or its session) has no text left: say nothing
                    let text = method(c, &sid, &block_id, "format", json!({ "format": "text", "trim": true }));
                    if !keep_open {
                        let _ = c.call("block.close", json!({ "session_id": sid, "block_id": block_id }));
                    }
                    if let Ok(r) = text {
                        // a reader gone (`| head`) must not cost the status: no println! panic
                        let _ = writeln!(std::io::stdout(), "{}", r["text"].as_str().unwrap_or_default());
                    }
                }
                return status;
            }
        }
        Cmd::Send { text } => {
            let (sid, bid) = pick_block(c, cli, None)?;
            let data = match text {
                Some(t) => t.clone(),
                None => std::io::read_to_string(std::io::stdin()).map_err(|e| Fail::Other(e.to_string()))?,
            };
            method(c, &sid, &bid, "write", json!({ "data": data }))?;
        }
        Cmd::SendKey { keys } => {
            let (sid, bid) = pick_block(c, cli, None)?;
            for key in keys {
                let event = json!({ "type": "key", "key": key });
                c.call("block.event", json!({ "session_id": sid, "block_id": bid, "event": event }))?;
            }
        }
        Cmd::Notify(NotifyArgs { state, kind, title, body }) => {
            if kind.is_some() && !matches!(state, NotifyState::Blocked) {
                return Err(Fail::Usage("--kind goes with --state blocked".into()));
            }
            // never the focused pane: a job outside leon would light up whatever the user looks at
            if block_query(cli, None).is_none() {
                return Err(Fail::Usage(
                    "no block to report for: pass -b BLOCK or run inside leon ($LEON_BLOCK)".into(),
                ));
            }
            let (sid, bid) = pick_block(c, cli, None)?;
            let name = |v: Option<clap::builder::PossibleValue>| v.map(|v| v.get_name().to_string());
            let (state, kind) = (name(state.to_possible_value()), name(kind.and_then(|k| k.to_possible_value())));
            method(
                c,
                &sid,
                &bid,
                "report_status",
                json!({ "state": state, "kind": kind, "title": title, "msg": body }),
            )?;
        }
        Cmd::Capture { trim } => {
            let (sid, bid) = pick_block(c, cli, None)?;
            let r = method(c, &sid, &bid, "format", json!({ "format": "text", "trim": trim }))?;
            println!("{}", r["text"].as_str().unwrap_or_default());
        }
        Cmd::Wait { target: block, timeout, text: Some(text) } => {
            let (sid, bid) = pick_block(c, cli, block.as_deref())?;
            return wait_text(c, &sid, &bid, text, *timeout);
        }
        Cmd::Wait { target: block, timeout, text: None } => {
            let query = block_query(cli, block.as_deref());
            let id = match pick_block(c, cli, block.as_deref()) {
                Ok((_, bid)) => bid,
                // A closed block no longer resolves, but the server still knows how it exited.
                Err(Fail::Api(e))
                    if e.code == "not_found" && query.as_deref().is_some_and(|q| q.starts_with("block:")) =>
                {
                    query.unwrap_or_default()
                }
                Err(f) => return Err(f),
            };
            return wait_for(c, &id, *timeout);
        }
        Cmd::Block(cmd) => block(c, cli, cmd)?,
        _ => unreachable!("commands::run routes only terminal commands here"),
    }
    Ok(())
}

fn block(c: &mut Client, cli: &Cli, cmd: &BlockCmd) -> Result<(), Fail> {
    match cmd {
        BlockCmd::Ls => {
            let sid = pick_session(c, cli, None)?;
            let r = c.call("session.list_blocks", json!({ "session_id": sid }))?;
            let blocks = r["blocks"].as_array().cloned().unwrap_or_default();
            if cli.json {
                print_json(&Value::Array(blocks));
            } else {
                for (i, b) in blocks.iter().enumerate() {
                    println!("{}  {}  {}", i + 1, summary(b), b["block_id"].as_str().unwrap_or(""));
                }
            }
        }
        BlockCmd::Inspect { target } => {
            let (sid, bid) = pick_block(c, cli, target.as_deref())?;
            let b = c.call("session.describe_block", json!({ "session_id": sid, "block_id": bid }))?;
            if cli.json {
                print_json(&b);
            } else {
                println!("{}  {}\n  window {}  session {sid}", summary(&b), bid, b["window_id"].as_str().unwrap_or(""));
                if let Some(r) = b["resume"].as_array() {
                    let argv: Vec<String> = r.iter().filter_map(Value::as_str).map(str::to_string).collect();
                    println!("  resume: {}", leon_core::keys::shell_line(&argv));
                }
            }
        }
        BlockCmd::Close { target } => {
            let (sid, bid) = pick_block(c, cli, target.as_deref())?;
            json_out(cli, &c.call("block.close", json!({ "session_id": sid, "block_id": bid }))?);
        }
        BlockCmd::Call { method: m, args } => {
            let (sid, bid) = pick_block(c, cli, None)?;
            let name = if m.contains('.') { m.clone() } else { format!("{TERMINAL}.{m}") };
            let args = read_args(args.as_deref())?;
            print_json(
                &c.call("block.method", json!({ "session_id": sid, "block_id": bid, "method": name, "args": args }))?,
            );
        }
    }
    Ok(())
}

/// `label  pid N  command…  [exited N]` for listings.
fn summary(b: &Value) -> String {
    let command: Vec<&str> = b["command"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
    let exited = b["exit_code"].as_i64().map(|c| format!("  exited {c}")).unwrap_or_default();
    format!("{}  pid {}  {}{exited}", label(&b["label"]), b["pid"], command.join(" "))
}

fn wait_for(c: &mut Client, block_id: &str, timeout: Option<Duration>) -> Result<(), Fail> {
    let timeout_ms = timeout.map(|d| d.as_millis() as u64);
    let r = c.call("block.wait", json!({ "block_id": block_id, "timeout_ms": timeout_ms }))?;
    match r["exit_code"].as_i64() {
        Some(0) => Ok(()),
        Some(code) => Err(Fail::Exit(code as i32)),
        None => Err(Fail::Other("the server sent no exit code".into())),
    }
}

/// Poll the block's text until it holds `text`. shortcut: reads the whole scrollback each time;
/// a server-side wait would be cheaper for a long one over a slow link.
fn wait_text(c: &mut Client, sid: &str, bid: &str, text: &str, timeout: Option<Duration>) -> Result<(), Fail> {
    let end = timeout.map(|t| std::time::Instant::now() + t);
    loop {
        match method(c, sid, bid, "format", json!({ "format": "text" })) {
            Ok(r) if r["text"].as_str().is_some_and(|t| t.contains(text)) => return Ok(()),
            Ok(_) => {}
            Err(Fail::Api(e)) if e.code == "not_found" => {
                return Err(Fail::Other(format!("the block closed before {text:?} showed")))
            }
            Err(f) => return Err(f),
        }
        if end.is_some_and(|end| std::time::Instant::now() >= end) {
            let waited = timeout.unwrap_or_default();
            return Err(Fail::Other(format!("{text:?} didn't show within {waited:?}")));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn method(c: &mut Client, sid: &str, bid: &str, name: &str, args: Value) -> Result<Value, Fail> {
    let m = format!("{TERMINAL}.{name}");
    c.call("block.method", json!({ "session_id": sid, "block_id": bid, "method": m, "args": args }))
}

/// True when nothing names a session and there is none, so `run` should create one.
fn no_session_named(c: &mut Client, cli: &Cli) -> Result<bool, Fail> {
    if cli.session.is_some() || env("LEON_SESSION").is_some() || env("LEON_BLOCK").is_some() {
        return Ok(false);
    }
    Ok(c.call("session.list", json!({}))?["sessions"].as_array().is_some_and(Vec::is_empty))
}

fn env(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|s| !s.is_empty())
}

/// The block query a command names: an explicit target, then `-b`, then `$LEON_BLOCK` (only when
/// `-s` names no session, since a name always beats the caller's own block).
pub(crate) fn block_query(cli: &Cli, explicit: Option<&str>) -> Option<String> {
    explicit
        .or(cli.block.as_deref())
        .map(str::to_string)
        .or_else(|| cli.session.is_none().then(|| env("LEON_BLOCK")).flatten())
}

/// The block a command acts on, as (session id, block id). A query is looked up in `-s`, else your
/// own session (`own_session`), else every session (full IDs always search every session unless `-s` is given);
/// with no query it is the active window's focused block in the picked session.
pub(crate) fn pick_block(c: &mut Client, cli: &Cli, explicit: Option<&str>) -> Result<(String, String), Fail> {
    if let Some(q) = block_query(cli, explicit) {
        let session = cli.session.clone().or_else(|| if q.starts_with("block:") { None } else { own_session(c) });
        let scope = match session {
            Some(s) => Some(resolve(c, "session", &s, None)?),
            None => None,
        };
        let r = c.call("target.resolve", json!({ "kind": "block", "query": q, "session_id": scope }))?;
        let field = |k: &str| r[k].as_str().unwrap_or_default().to_string();
        return Ok((field("session_id"), field("id")));
    }
    let sid = pick_session(c, cli, None)?;
    let v = c.call("session.view", json!({ "session_id": sid }))?;
    let windows = v["windows"].as_array().cloned().unwrap_or_default();
    let active = windows.iter().find(|w| w["active"] == true);
    match active.and_then(|w| w["focused_block_id"].as_str()) {
        Some(bid) => Ok((sid, bid.to_string())),
        None => Err(Fail::Api(ApiError::not_found("the active window has no block; pass -b"))),
    }
}

/// `500ms`, `10s`, `2m`, `1h`, or bare seconds.
pub fn parse_duration(s: &str) -> Result<Duration, String> {
    let s = s.trim();
    let split = s.find(|c: char| !c.is_ascii_digit() && c != '.').unwrap_or(s.len());
    let (num, unit) = s.split_at(split);
    let n: f64 = num.parse().map_err(|_| format!("not a duration: {s:?}"))?;
    let secs = match unit {
        "ms" => n / 1000.0,
        "" | "s" => n,
        "m" => n * 60.0,
        "h" => n * 3600.0,
        _ => return Err(format!("unknown unit in {s:?}; use ms, s, m or h")),
    };
    Duration::try_from_secs_f64(secs).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::parse_duration;
    use std::time::Duration;

    #[test]
    fn durations() {
        assert_eq!(parse_duration("200ms"), Ok(Duration::from_millis(200)));
        assert_eq!(parse_duration("10s"), Ok(Duration::from_secs(10)));
        assert_eq!(parse_duration("3"), Ok(Duration::from_secs(3)));
        assert_eq!(parse_duration("1.5m"), Ok(Duration::from_secs(90)));
        assert!(parse_duration("5 parsecs").is_err());
        assert!(parse_duration("").is_err());
    }
}
