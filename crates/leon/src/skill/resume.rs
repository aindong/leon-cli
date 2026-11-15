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
    let Some(block) = var("LEON_BLOCK") else { return };
    let Some(command) = stdin_json().and_then(|v| command(agent, &v)) else { return };
    if !interactive() || (agent == Agent::Claude && print_mode()) {
        return;
    }
    let socket = var("LEON_SERVER").map_or_else(leon_core::paths::socket_path, PathBuf::from);
    let Ok(mut c) = Client::connect(&socket) else { return };
    if c.set_timeout(Some(SERVER_WAIT)).is_err() {
        return;
    }
    let Some(session) = crate::commands::own_session(&mut c) else { return };
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

/// Whether this is Claude Code's print mode (`claude -p`, `--print`): a one-shot run, with nothing
/// to resume. Read from the argv of the hook, its parent and its grandparent (the agent, under the
/// hook's shell), not from a program name: an npm install runs as `node …/claude -p`.
fn print_mode() -> bool {
    // SAFETY: getpid has no preconditions.
    let mut pid = unsafe { libc::getpid() };
    for _ in 0..3 {
        if argv(pid).is_some_and(|a| print_flag(&a)) {
            return true;
        }
        match process(pid) {
            Some(p) => pid = p.ppid,
            None => return false,
        }
    }
    false
}

/// `-p` (also among short flags written together, `-cp`), `--print` or `--print=…` among the
/// options, before any `--`.
fn print_flag(argv: &[String]) -> bool {
    let short =
        |a: &str| a.strip_prefix('-').is_some_and(|f| f.contains('p') && f.chars().all(|c| c.is_ascii_alphabetic()));
    argv.iter().skip(1).take_while(|a| *a != "--").any(|a| short(a) || a == "--print" || a.starts_with("--print="))
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

/// A process's argv: `KERN_PROCARGS2` is argc, the exec path, NUL padding, then the arguments.
#[cfg(target_os = "macos")]
fn argv(pid: i32) -> Option<Vec<String>> {
    let mut mib = [libc::CTL_KERN, libc::KERN_PROCARGS2, pid];
    let mut size: libc::size_t = 0;
    let null = std::ptr::null_mut();
    // SAFETY: a size query, then a read of at most `size` bytes into a buffer that long.
    let mut buf = unsafe {
        if libc::sysctl(mib.as_mut_ptr(), 3, null, &mut size, null, 0) != 0 {
            return None;
        }
        let mut buf = vec![0u8; size];
        if libc::sysctl(mib.as_mut_ptr(), 3, buf.as_mut_ptr().cast(), &mut size, null, 0) != 0 {
            return None;
        }
        buf
    };
    buf.truncate(size);
    let argc = usize::try_from(i32::from_ne_bytes(buf.get(..4)?.try_into().ok()?)).ok()?;
    let rest = &buf[4..];
    let rest = &rest[rest.iter().position(|&b| b == 0)?..];
    let rest = &rest[rest.iter().position(|&b| b != 0)?..];
    Some(rest.split(|&b| b == 0).take(argc).map(|a| String::from_utf8_lossy(a).into_owned()).collect())
}

#[cfg(target_os = "linux")]
fn argv(pid: i32) -> Option<Vec<String>> {
    let raw = std::fs::read(format!("/proc/{pid}/cmdline")).ok()?;
    Some(raw.split(|&b| b == 0).filter(|a| !a.is_empty()).map(|a| String::from_utf8_lossy(a).into_owned()).collect())
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn argv(_pid: i32) -> Option<Vec<String>> {
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

    #[test]
    fn print_flags_before_a_double_dash() {
        let v = |s: &str| s.split(' ').map(String::from).collect::<Vec<_>>();
        for yes in
            ["claude -p hi", "node /x/claude --print hi", "claude --print=json", "claude --model m -p", "claude -cp q"]
        {
            assert!(print_flag(&v(yes)), "{yes}");
        }
        for no in ["claude", "claude --resume abc", "claude -- -p", "-p", "claude -c", "claude --printer", "claude -"] {
            assert!(!print_flag(&v(no)), "{no}");
        }
    }

    #[test]
    fn reads_its_own_argv() {
        let me = argv(std::process::id() as i32).expect("our own argv");
        assert_eq!(me, std::env::args().collect::<Vec<_>>());
    }
}
