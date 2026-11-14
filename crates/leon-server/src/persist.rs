//! Sessions saved when the server stops and restored when it starts (docs/spec.md §1). Programs
//! don't survive a restart: a restored block runs a fresh login shell in its saved cwd, and the
//! saved command is kept only for the record, so a restart never re-runs a command.
//!
//! The file lives in a private dir (0700, ours), is written 0600 through a temp file and a rename,
//! and a file that can't be used is moved aside with a warning, never replaced.

use std::collections::HashSet;
use std::fs::{self, OpenOptions};
use std::io::{self, Read, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use leon_core::layout::Node;
use leon_core::model::clean_label;
use leon_core::panes::check_ratio;
use leon_core::{ApiError, Block, Session, State, Window};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::Ctx;

pub const FILE: &str = "sessions.json";
const VERSION: u64 = 1;
/// A bigger file is taken as corrupt.
const MAX_FILE: u64 = 16 << 20;
/// More blocks than this (the spec's server limit) is taken as corrupt: each would start a shell.
const MAX_BLOCKS: usize = 1024;
/// A revision past this is corrupt; the model only ever adds one.
const MAX_REVISION: u64 = 1 << 53;

/// Where to save, and whether to restore what is there first.
#[derive(Debug, Clone)]
pub struct Persist {
    pub file: PathBuf,
    pub restore: bool,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Saved {
    version: u64,
    sessions: Vec<SavedSession>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SavedSession {
    session_id: String,
    label: Option<String>,
    revision: u64,
    /// Index into `windows` of the active tab.
    active: usize,
    windows: Vec<SavedWindow>,
    /// In creation order.
    blocks: Vec<SavedBlock>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SavedWindow {
    window_id: String,
    label: Option<String>,
    layout: Node,
    focus: Option<String>,
    zoomed: Option<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SavedBlock {
    block_id: String,
    label: Option<String>,
    window_id: String,
    keep_open: bool,
    cwd: Option<PathBuf>,
    /// What ran; not run again.
    command: Vec<String>,
}

/// Check the state dir is private and no other server uses it, and restore from it if asked.
/// Returns the file to save to and the lock that keeps the dir ours (hold it until saved), or
/// None, with a warning, when the dir isn't safe or is taken.
pub fn start(ctx: &Arc<Ctx>, p: &Persist) -> Option<(PathBuf, fs::File)> {
    let dir = p.file.parent().unwrap_or(Path::new("."));
    let taken = leon_core::paths::ensure_private_dir(dir).and_then(|_| lock(dir));
    let lock = match taken {
        Ok(lock) => lock,
        Err(e) => {
            eprintln!("leon: warning: sessions won't be saved or restored: {e}");
            return None;
        }
    };
    if p.restore && !restore(ctx, &p.file) {
        eprintln!("leon: warning: sessions won't be saved, so {} stays as it is", p.file.display());
        return None;
    }
    Some((p.file.clone(), lock))
}

/// An exclusive flock on the dir itself, or an error if another server (another runtime dir) holds it.
fn lock(dir: &Path) -> io::Result<fs::File> {
    use std::os::unix::io::AsRawFd;
    let f = OpenOptions::new().read(true).custom_flags(libc::O_NOFOLLOW | libc::O_DIRECTORY).open(dir)?;
    // SAFETY: flock on a file descriptor we own.
    if unsafe { libc::flock(f.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
        let e = io::Error::last_os_error();
        return Err(match e.kind() {
            io::ErrorKind::WouldBlock => io::Error::other(format!("another leon server uses {}", dir.display())),
            _ => e,
        });
    }
    Ok(f)
}

/// Write the sessions to `file`: a 0600 temp file beside it, synced, then renamed over it.
pub fn save(st: &State, file: &Path) -> io::Result<()> {
    let bytes = serde_json::to_vec_pretty(&snapshot(st)).map_err(io::Error::other)?;
    let tmp = file.with_extension("json.tmp");
    let _ = fs::remove_file(&tmp); // a crash's leftover; the dir is ours
    let written = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(&tmp)
        .and_then(|mut f| f.write_all(&bytes).and_then(|_| f.sync_all()))
        .and_then(|_| fs::rename(&tmp, file));
    if written.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    written?;
    fs::File::open(file.parent().unwrap_or(Path::new(".")))?.sync_all() // the rename itself
}

/// Forget the saved sessions (`server stop --discard`).
pub fn discard(file: &Path) -> io::Result<()> {
    match fs::remove_file(file) {
        Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    }
}

fn snapshot(st: &State) -> Saved {
    let sessions = st.sessions.iter().map(|s| SavedSession {
        session_id: s.id.clone(),
        label: s.label.clone(),
        revision: s.revision,
        active: s.active,
        windows: s
            .windows
            .iter()
            .filter_map(|w| {
                Some(SavedWindow {
                    window_id: w.id.clone(),
                    label: w.label.clone(),
                    layout: w.layout.clone()?, // None only while a call opens its first block
                    focus: w.focus.clone(),
                    zoomed: w.zoomed.clone(),
                })
            })
            .collect(),
        blocks: s
            .blocks
            .iter()
            .map(|b| SavedBlock {
                block_id: b.id.clone(),
                label: b.label.clone(),
                window_id: b.window_id.clone(),
                keep_open: b.keep_open,
                cwd: saveable(b.terminal.cwd()),
                command: b.terminal.command.clone(),
            })
            .collect(),
    });
    Saved { version: VERSION, sessions: sessions.collect() }
}

/// JSON holds only UTF-8 paths; a shell in any other dir restores in `$HOME`.
fn saveable(cwd: Option<PathBuf>) -> Option<PathBuf> {
    cwd.filter(|d| d.to_str().is_some())
}

/// Bring back the saved sessions. A file that can't be used is moved aside; so is one with a
/// session that couldn't be restored, so the next save can't lose that session. False if such a
/// file couldn't be moved, so saving over it would lose it.
fn restore(ctx: &Arc<Ctx>, file: &Path) -> bool {
    let saved = match load(file) {
        Ok(None) => return true,
        Ok(Some(saved)) => saved,
        Err(why) => return aside(file, &format!("{why}; starting without its sessions")),
    };
    // Held while spawning, so an instant exit can't be reported before its block is in the model.
    let mut st = ctx.state();
    let mut skipped = false;
    for s in saved.sessions {
        let id = s.session_id.clone();
        match restore_session(ctx, s) {
            Ok(session) => st.sessions.push(session),
            Err(e) => {
                eprintln!("leon: warning: can't restore {id}: {e}");
                skipped = true;
            }
        }
    }
    !skipped || aside(file, "a session in it couldn't be restored")
}

fn restore_session(ctx: &Arc<Ctx>, s: SavedSession) -> Result<Session, ApiError> {
    let SavedSession { session_id, label, revision, active, windows, blocks } = s;
    let blocks = blocks
        .into_iter()
        .map(|b| {
            // A dir that has gone starts the shell in $HOME instead.
            let cwd = b.cwd.filter(|d| d.is_dir());
            let terminal = crate::blocks::spawn(ctx, &session_id, &b.block_id, vec![], cwd)?;
            let (id, label, window_id, keep_open) = (b.block_id, b.label, b.window_id, b.keep_open);
            Ok(Block { id, label, window_id, keep_open, exit_code: None, terminal: Arc::new(terminal) })
        })
        .collect::<Result<Vec<_>, ApiError>>()?;
    let windows = windows
        .into_iter()
        .map(|w| Window { id: w.window_id, label: w.label, layout: Some(w.layout), focus: w.focus, zoomed: w.zoomed })
        .collect();
    Ok(Session { id: session_id, label, windows, active, blocks, revision })
}

/// The saved sessions, None if there is no file, or why the file can't be used.
fn load(file: &Path) -> Result<Option<Saved>, String> {
    // O_NONBLOCK: opening a FIFO planted at the path must not hang the server.
    let f = match OpenOptions::new().read(true).custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK).open(file) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("can't read it: {e}")),
        Ok(f) => f,
    };
    if !f.metadata().is_ok_and(|m| m.is_file()) {
        return Err("it isn't a regular file".into());
    }
    let mut bytes = Vec::new();
    f.take(MAX_FILE + 1).read_to_end(&mut bytes).map_err(|e| format!("can't read it: {e}"))?;
    if bytes.len() as u64 > MAX_FILE {
        return Err(format!("it is over {} MiB", MAX_FILE >> 20));
    }
    let v: Value = serde_json::from_slice(&bytes).map_err(|e| format!("it isn't JSON: {e}"))?;
    match v.get("version").and_then(Value::as_u64) {
        Some(VERSION) => {}
        Some(n) if n > VERSION => return Err(format!("it was saved by a newer leon (version {n})")),
        _ => return Err("it has no version this leon knows".into()),
    }
    let saved: Saved = serde_json::from_value(v).map_err(|e| format!("it isn't saved sessions: {e}"))?;
    check(&saved)?;
    Ok(Some(saved))
}

/// Everything the model relies on: unique well-formed IDs, clean labels, ratios in (0, 1), each
/// window's tree holding exactly its blocks, and focus, zoom and the active tab pointing inside.
fn check(saved: &Saved) -> Result<(), String> {
    let mut seen = HashSet::new();
    let mut fresh = |id: &str, kind: &str| {
        let rest = id.strip_prefix(kind).and_then(|r| r.strip_prefix(':')).unwrap_or("");
        let ok = !rest.is_empty() && rest.len() <= 64 && rest.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-');
        match (ok, seen.insert(id.to_string())) {
            (false, _) => Err(format!("{id:?} isn't a {kind} ID")),
            (_, false) => Err(format!("{id} appears twice")),
            _ => Ok(()),
        }
    };
    if saved.sessions.iter().map(|s| s.blocks.len()).sum::<usize>() > MAX_BLOCKS {
        return Err(format!("it has over {MAX_BLOCKS} blocks"));
    }
    for s in &saved.sessions {
        fresh(&s.session_id, "session")?;
        if s.revision > MAX_REVISION {
            return Err(format!("{} has revision {}", s.session_id, s.revision));
        }
        label(&s.label)?;
        if s.active >= s.windows.len() {
            return Err(format!("{} has no window {}", s.session_id, s.active + 1));
        }
        for b in &s.blocks {
            fresh(&b.block_id, "block")?;
            label(&b.label)?;
        }
        for w in &s.windows {
            fresh(&w.window_id, "window")?;
            label(&w.label)?;
            splits(&w.layout, &mut fresh)?;
            let mut leaves = w.layout.leaves();
            let mut own: Vec<&str> =
                s.blocks.iter().filter(|b| b.window_id == w.window_id).map(|b| b.block_id.as_str()).collect();
            leaves.sort_unstable();
            own.sort_unstable();
            if leaves != own {
                return Err(format!("{}'s layout doesn't hold exactly its blocks", w.window_id));
            }
            for id in [&w.focus, &w.zoomed].into_iter().flatten() {
                if !leaves.contains(&id.as_str()) {
                    return Err(format!("{id:?} isn't in {}", w.window_id));
                }
            }
        }
        let placed: usize = s.windows.iter().map(|w| w.layout.leaves().len()).sum();
        if placed != s.blocks.len() {
            return Err(format!("{} has blocks in no window", s.session_id));
        }
    }
    Ok(())
}

fn label(l: &Option<String>) -> Result<(), String> {
    match l {
        Some(l) if clean_label(Some(l)).ok().flatten().as_deref() != Some(l.as_str()) => {
            Err(format!("bad label {l:?}"))
        }
        _ => Ok(()),
    }
}

fn splits(n: &Node, fresh: &mut impl FnMut(&str, &str) -> Result<(), String>) -> Result<(), String> {
    if let Node::Split { split_id, ratio, before, after, .. } = n {
        fresh(split_id, "split")?;
        check_ratio(*ratio).map_err(|e| e.message)?;
        splits(before, fresh)?;
        splits(after, fresh)?;
    }
    Ok(())
}

/// Move `file` to `<file>.bad-<secs>-<n>`, a name nothing has yet (a hard link never replaces),
/// and warn. False if it couldn't be moved.
fn aside(file: &Path, why: &str) -> bool {
    // `why` may quote the file (serde names unknown keys); keep its control characters off the log.
    let why: String = why.chars().map(|c| if c.is_control() { '?' } else { c }).collect();
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
    for n in 0..1000 {
        let to = PathBuf::from(format!("{}.bad-{secs}-{n}", file.display()));
        match fs::hard_link(file, &to) {
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(e) => {
                eprintln!("leon: warning: {}: {why}; can't move it aside: {e}", file.display());
                return false;
            }
            Ok(()) => {
                let _ = fs::remove_file(file);
                eprintln!("leon: warning: {}: {why}; moved it to {}", file.display(), to.display());
                return true;
            }
        }
    }
    eprintln!("leon: warning: {}: {why}; no free name to move it aside", file.display());
    false
}

#[cfg(test)]
#[path = "persist_tests.rs"]
mod tests;
