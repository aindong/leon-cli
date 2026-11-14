//! What a restored block shows and runs (M14). Its saved text comes back inert and capped, above a
//! dim "— restored from <time> —" line; the resume command a program registered for itself
//! (`set_resume`) is typed at the new shell's prompt by the `resume` setting. Nothing else that ran
//! is run again.

use std::sync::{Arc, Weak};
use std::time::{Duration, Instant};

use leon_core::terminal::Terminal;
pub use leon_lua::Resume;

/// Saved text per block, and over all blocks (each gets an equal share), in bytes.
pub const PER_BLOCK: usize = 2 << 20;
pub const TOTAL: usize = 8 << 20;
const MAX_ARGS: usize = 64;
const MAX_COMMAND: usize = 8 << 10;
/// How long a restored shell gets to show its prompt before its resume command is dropped.
const PROMPT_WAIT: Duration = Duration::from_secs(10);
const DIM: &str = "\x1b[2m";
const PLAIN: &str = "\x1b[0m";

/// Each of `blocks` blocks' share of the cap.
pub fn cap(blocks: usize) -> usize {
    PER_BLOCK.min(TOTAL / blocks.max(1))
}

/// `text` with every control character but `\n` removed (C0, DEL, C1: no escape, no carriage
/// return), cut to its newest whole lines within `cap` bytes.
pub fn inert(text: &str, cap: usize) -> String {
    let clean: String = text.chars().filter(|c| *c == '\n' || !c.is_control()).collect();
    if clean.len() <= cap {
        return clean;
    }
    let mut start = clean.len() - cap;
    while !clean.is_char_boundary(start) {
        start += 1;
    }
    if clean.as_bytes()[start - 1] != b'\n' {
        // Drop the partial line; a single line longer than the cap keeps its tail.
        start = clean[start..].find('\n').map_or(start, |i| start + i + 1);
    }
    clean[start..].to_string()
}

/// A resume command a program may register: an argv with a program name, at most 64 arguments and
/// 8 KiB, and no control characters (typing it at a prompt must not press Enter or send a key).
pub fn check_command(command: &[String]) -> Result<(), String> {
    match command.first() {
        None => return Err("command needs at least a program name".into()),
        Some(p) if p.is_empty() => return Err("the program name is empty".into()),
        _ => {}
    }
    if command.len() > MAX_ARGS {
        return Err(format!("command has over {MAX_ARGS} arguments"));
    }
    if command.iter().map(String::len).sum::<usize>() > MAX_COMMAND {
        return Err(format!("command is over {} KiB", MAX_COMMAND >> 10));
    }
    if command.iter().any(|a| a.chars().any(char::is_control)) {
        return Err("command has a control character".into());
    }
    Ok(())
}

/// What the new terminal shows before its shell starts: the saved text, then the dim separator
/// (and under "ask" a hint). Nothing when there is neither text nor a command to offer.
pub fn shown(text: Option<&str>, cap: usize, saved_at: Option<u64>, offer: Option<Resume>) -> Vec<u8> {
    let text = text.map(|t| inert(t, cap)).filter(|t| !t.is_empty());
    let offer = offer.filter(|p| *p != Resume::Never);
    if text.is_none() && offer.is_none() {
        return vec![];
    }
    let mut out = String::new();
    if let Some(t) = text {
        out.push_str(&t.replace('\n', "\r\n"));
        out.push_str("\r\n");
    }
    let when = saved_at.map_or_else(|| "an earlier server".to_string(), local_time);
    out.push_str(&format!("{DIM}— restored from {when} —{PLAIN}\r\n"));
    if offer == Some(Resume::Ask) {
        out.push_str(&format!("{DIM}— press Enter to resume, Ctrl-C to skip —{PLAIN}\r\n"));
    }
    out.into_bytes()
}

/// Type `command` at `t`'s prompt once its shell reads a line (Enter too under "always"); the line
/// is cleared first so nothing typed before can join it. Gives up after `PROMPT_WAIT`, or once the
/// block is gone. Ctrl-U clears the line in canonical mode, readline, zle and fish.
pub fn offer(t: &Arc<Terminal>, command: &[String], policy: Resume) {
    if policy == Resume::Never {
        return;
    }
    let mut keys = format!("\x15{}", leon_core::terminal::shell_line(command));
    if policy == Resume::Always {
        keys.push('\r');
    }
    let weak: Weak<Terminal> = Arc::downgrade(t);
    std::thread::spawn(move || {
        let end = Instant::now() + PROMPT_WAIT;
        while Instant::now() < end {
            let Some(t) = weak.upgrade() else { return };
            if t.reads_a_line() {
                let _ = t.write(keys.as_bytes());
                return;
            }
            drop(t);
            std::thread::sleep(Duration::from_millis(20));
        }
    });
}

/// `YYYY-MM-DD HH:MM` in local time.
fn local_time(secs: u64) -> String {
    let t = secs as libc::time_t;
    // SAFETY: localtime_r only writes into our local.
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    if unsafe { libc::localtime_r(&t, &mut tm) }.is_null() {
        return "an earlier server".into();
    }
    format!("{:04}-{:02}-{:02} {:02}:{:02}", tm.tm_year + 1900, tm.tm_mon + 1, tm.tm_mday, tm.tm_hour, tm.tm_min)
}
