//! The SessionStart hook's second job (M14): record how to resume the agent's session for its own
//! block (`dev.leon.terminal.set_resume`), so a restored block can offer it. Every failure is
//! silent and bounded: the hook must never fail or stall the agent's start.

use std::io::{IsTerminal, Read};
use std::path::PathBuf;
use std::time::Duration;

use serde_json::{json, Value};

use super::Agent;
use crate::client::Client;

/// How long the hook waits for its stdin, then for the server's reply: about 2 s in all.
const STDIN_WAIT: Duration = Duration::from_secs(1);
const SERVER_WAIT: Duration = Duration::from_secs(1);
/// A SessionStart input is a few hundred bytes.
const MAX_INPUT: u64 = 64 << 10;

/// The agent's resume command, for its SessionStart JSON. Claude Code 2.1: `claude --resume <id>`;
/// Codex 0.164: `codex resume <id>`. Both ids are UUIDs.
fn command(agent: Agent, input: &Value) -> Option<Vec<String>> {
    let id = input["session_id"].as_str()?;
    let ok = !id.is_empty()
        && id.len() <= 128
        && !id.starts_with('-')
        && id.bytes().all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b));
    if !ok {
        return None;
    }
    let argv: &[&str] = match agent {
        Agent::Claude => &["claude", "--resume", id],
        Agent::Codex => &["codex", "resume", id],
    };
    Some(argv.iter().map(|s| s.to_string()).collect())
}

pub fn record(agent: Agent) {
    let var = |n| std::env::var(n).ok().filter(|v| !v.is_empty());
    let (Some(session), Some(block)) = (var("LEON_SESSION"), var("LEON_BLOCK")) else { return };
    let Some(command) = stdin_json().and_then(|v| command(agent, &v)) else { return };
    if !interactive() {
        return;
    }
    let socket = var("LEON_SERVER").map_or_else(leon_core::paths::socket_path, PathBuf::from);
    let Ok(mut c) = Client::connect(&socket) else { return };
    if c.set_timeout(Some(SERVER_WAIT)).is_err() {
        return;
    }
    let args = json!({ "command": command });
    let params =
        json!({ "session_id": session, "block_id": block, "method": "dev.leon.terminal.set_resume", "args": args });
    let _ = c.call("block.method", params);
}

/// The hook's stdin as JSON, read on a thread so an agent that never closes it can't hold us.
fn stdin_json() -> Option<Value> {
    if std::io::stdin().is_terminal() {
        return None;
    }
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let read = std::io::stdin().take(MAX_INPUT).read_to_end(&mut bytes);
        let _ = tx.send(read.ok().map(|_| bytes));
    });
    serde_json::from_slice(&rx.recv_timeout(STDIN_WAIT).ok()??).ok()
}

/// Whether the hook belongs to an agent a person is using: the hook, its parent or its grandparent
/// is the foreground process group of its terminal. Claude Code runs a hook's shell in a session of
/// its own, under the interactive `claude`; an agent started by another agent's tool (`claude -p`
/// from a Bash tool) has no terminal, so it can't replace the interactive session's command.
fn interactive() -> bool {
    // SAFETY: getpid has no preconditions.
    let mut pid = unsafe { libc::getpid() };
    for _ in 0..3 {
        let Some(p) = process(pid) else { return false };
        if p.tpgid > 0 && p.pgid == p.tpgid {
            return true;
        }
        pid = p.ppid;
    }
    false
}

struct Process {
    ppid: i32,
    pgid: i32,
    /// The foreground process group of its terminal; 0 or less without one.
    tpgid: i32,
}

#[cfg(target_os = "macos")]
fn process(pid: i32) -> Option<Process> {
    let size = std::mem::size_of::<libc::proc_bsdinfo>() as libc::c_int;
    // SAFETY: proc_pidinfo writes at most `size` bytes into our zeroed struct.
    let info = unsafe {
        let mut info: libc::proc_bsdinfo = std::mem::zeroed();
        let ptr = &mut info as *mut libc::proc_bsdinfo as *mut libc::c_void;
        (libc::proc_pidinfo(pid, libc::PROC_PIDTBSDINFO, 0, ptr, size) == size).then_some(info)?
    };
    Some(Process { ppid: info.pbi_ppid as i32, pgid: info.pbi_pgid as i32, tpgid: info.e_tpgid as i32 })
}

#[cfg(target_os = "linux")]
fn process(pid: i32) -> Option<Process> {
    // `pid (comm) state ppid pgrp session tty_nr tpgid …`; comm may hold spaces and parens.
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let f: Vec<i32> =
        stat[stat.rfind(')')? + 2..].split(' ').skip(1).take(5).map(|n| n.parse().ok()).collect::<Option<_>>()?;
    Some(Process { ppid: f[0], pgid: f[1], tpgid: f[4] })
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn process(_pid: i32) -> Option<Process> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_plain_session_ids_make_a_command() {
        let input = |id: &str| json!({ "session_id": id });
        let id = "c7583d2c-3133-4d13-b5f0-45883f1cabcb";
        assert_eq!(command(Agent::Claude, &input(id)).unwrap(), ["claude", "--resume", id]);
        assert_eq!(command(Agent::Codex, &input(id)).unwrap(), ["codex", "resume", id]);
        for bad in ["", "-r", "a b", "a;b", "a\nb", "ü", &"a".repeat(129)] {
            assert_eq!(command(Agent::Claude, &input(bad)), None, "{bad:?}");
        }
        assert_eq!(command(Agent::Claude, &json!({ "session_id": 5 })), None);
    }
}
