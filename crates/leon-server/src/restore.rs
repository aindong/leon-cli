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
/// How long a restored shell gets to show its prompt before its resume command is dropped: long
/// enough for the user to answer a question the shell's rc asks first.
const PROMPT_WAIT: Duration = Duration::from_secs(60);
/// Shells whose line editor turns bracketed paste on, unlike a `read` in their rc files.
const PASTING_SHELLS: [&str; 2] = ["zsh", "fish"];
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
/// 8 KiB, of printable ASCII but `\\`. Under "ask" the user agrees to what they see typed, so it
/// must be what runs: no key it could press (controls), no line that looks like another (bidi,
/// invisible), no bytes readline might read as Meta keys, no backslash (fish reads `\\'` inside
/// single quotes).
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
    if command.iter().any(|a| a.chars().any(|c| !(' '..='~').contains(&c) || c == '\\')) {
        return Err("command may hold only printable ASCII, and no backslash".into());
    }
    Ok(())
}

/// What one restore applies to every block.
pub struct Restoring {
    /// Each block's share of the saved text (`cap`).
    pub cap: usize,
    /// When the file was saved, in seconds since the epoch.
    pub saved_at: Option<u64>,
    pub policy: Resume,
}

/// What the new terminal shows before its shell starts: the saved text, then the dim separator
/// (and under "ask" a hint, if the block has a command to offer). Nothing when there is neither
/// text nor a command to offer.
pub fn shown(text: Option<&str>, offering: bool, how: &Restoring) -> Vec<u8> {
    let text = text.map(|t| inert(t, how.cap)).filter(|t| !t.is_empty());
    let offer = Some(how.policy).filter(|p| offering && *p != Resume::Never);
    if text.is_none() && offer.is_none() {
        return vec![];
    }
    let mut out = String::new();
    if let Some(t) = text {
        out.push_str(&t.replace('\n', "\r\n"));
        out.push_str("\r\n");
    }
    let when = how.saved_at.and_then(local_time).unwrap_or_else(|| "an earlier server".into());
    out.push_str(&format!("{DIM}— restored from {when} —{PLAIN}\r\n"));
    if offer == Some(Resume::Ask) {
        out.push_str(&format!("{DIM}— press Enter to resume, Ctrl-C to skip —{PLAIN}\r\n"));
    }
    out.into_bytes()
}

/// Type `command` at `t`'s prompt once its shell reads a line (Enter too under "always"); the line
/// is cleared first so nothing typed before can join it. Gives up after `PROMPT_WAIT`, or once the
/// block is gone. Ctrl-U clears the line in canonical mode, readline, zle and fish. zsh and fish
/// must also have turned bracketed paste on, which their line editors do and an rc's `read -k`
/// (oh-my-zsh's update question, compinit's `read -q`) doesn't.
// shortcut: bash and sh are taken at ICANON alone, so a key an rc reads with canonical mode off gets
// the keys; bash 5.1+ turns bracketed paste on too, so check its version if that matters.
pub fn offer(t: &Arc<Terminal>, command: &[String], policy: Resume) {
    if policy == Resume::Never {
        return;
    }
    let mut keys = format!("\x15{}", leon_core::keys::shell_line(command));
    if policy == Resume::Always {
        keys.push('\r');
    }
    let shell = t.command.first().and_then(|c| c.rsplit('/').next()).unwrap_or("").trim_start_matches('-');
    let pasting = PASTING_SHELLS.contains(&shell);
    let weak: Weak<Terminal> = Arc::downgrade(t);
    std::thread::spawn(move || {
        let end = Instant::now() + PROMPT_WAIT;
        while Instant::now() < end {
            let Some(t) = weak.upgrade() else { return };
            if t.reads_a_line() && (!pasting || t.bracketed_paste()) {
                let _ = t.write(keys.as_bytes());
                return;
            }
            drop(t);
            std::thread::sleep(Duration::from_millis(20));
        }
    });
}

/// `YYYY-MM-DD HH:MM` in local time.
fn local_time(secs: u64) -> Option<String> {
    let t = secs as libc::time_t;
    // SAFETY: localtime_r only writes into our local.
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    if unsafe { libc::localtime_r(&t, &mut tm) }.is_null() {
        return None;
    }
    Some(format!(
        "{:04}-{:02}-{:02} {:02}:{:02}",
        i64::from(tm.tm_year) + 1900,
        tm.tm_mon + 1,
        tm.tm_mday,
        tm.tm_hour,
        tm.tm_min
    ))
}
