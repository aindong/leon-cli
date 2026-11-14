//! Screens pushed to a connection (`block.watch`). Each watched block has a task that waits for
//! its terminal to change and marks it dirty; the connection loop builds the frame, diffs it
//! against the last one it sent and writes one `{"screen":…}` line. Nothing queues: a block is
//! dirty or not, so a client that reads slowly gets fewer, newer screens and never a backlog.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Weak};
use std::time::{Duration, Instant};

use leon_core::screen::{self, Frame, Update};
use leon_core::terminal::Terminal;
use leon_core::{theme, ApiError};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::sync::Notify;

use crate::methods::Empty;
use crate::registry::parse;
use crate::{Ctx, Registry};

/// The least time between two screens of one block: a flood of output costs at most this many
/// frame builds (each holds the terminal's lock), while the first change after a quiet spell (a
/// keystroke's echo) goes at once.
const MIN_GAP: Duration = Duration::from_millis(2);
/// The most blocks one connection may watch.
const MAX_WATCHED: usize = 256;

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Watch {
    /// The blocks whose screens this connection gets, replacing the last set; `[]` stops them all.
    pub block_ids: Vec<String>,
}

/// One `screen` line, written straight to bytes (no `Value` tree per push).
#[derive(Serialize)]
struct Line<'a> {
    screen: Pushed<'a>,
}

/// An update for one block.
#[derive(Serialize)]
struct Pushed<'a> {
    block_id: &'a str,
    #[serde(flatten)]
    update: Update,
}

struct Watched {
    /// Weak: watching a block must not keep its program running after the block closes.
    term: Weak<Terminal>,
    dirty: Arc<AtomicBool>,
    sent: Option<Frame>,
    last: Option<Instant>,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for Watched {
    fn drop(&mut self) {
        self.task.abort();
    }
}

/// A connection's watched blocks.
#[derive(Default)]
pub struct Screens {
    watched: HashMap<String, Watched>,
    /// Woken when a watched block changes.
    pub ready: Arc<Notify>,
}

/// The screen `t` shows now.
pub fn frame(t: &Terminal) -> Frame {
    // shortcut: colours are resolved with leon's one theme; a client sends its own once themes exist (spec `set_theme`).
    t.with_term(|term| screen::frame(term, &theme::DARK))
}

fn terminals(ctx: &Ctx, ids: &[String]) -> Result<Vec<Arc<Terminal>>, ApiError> {
    if ids.len() > MAX_WATCHED {
        return Err(ApiError::bad_request(format!("at most {MAX_WATCHED} blocks")));
    }
    let st = ctx.state();
    ids.iter().map(|id| Ok(st.block(id)?.1.terminal.clone())).collect()
}

impl Screens {
    /// Replace the watched set. Blocks already watched keep what was sent (the client has it);
    /// new ones get a whole screen first.
    pub fn watch(&mut self, ctx: &Ctx, params: Value) -> Result<Value, ApiError> {
        let p: Watch = parse("block.watch", params)?;
        let terms = terminals(ctx, &p.block_ids)?;
        let mut keep = HashMap::new();
        for (id, t) in p.block_ids.into_iter().zip(terms) {
            if let Some(w) = self.watched.remove(&id) {
                keep.insert(id, w);
                continue;
            }
            let dirty = Arc::new(AtomicBool::new(true));
            let (mut changes, flag, ready) = (t.subscribe(), dirty.clone(), self.ready.clone());
            let task = tokio::spawn(async move {
                while changes.changed().await.is_ok() {
                    flag.store(true, Ordering::Release);
                    ready.notify_one();
                }
            });
            keep.insert(id, Watched { term: Arc::downgrade(&t), dirty, sent: None, last: None, task });
        }
        self.watched = keep;
        self.ready.notify_one();
        Ok(json!({}))
    }

    /// The screen lines due now, as encoded lines. A dirty block whose last screen went less than
    /// `MIN_GAP` ago waits for `due`.
    pub fn take(&mut self) -> Vec<Vec<u8>> {
        let now = Instant::now();
        let mut out = Vec::new();
        self.watched.retain(|_, w| w.term.strong_count() > 0);
        for (id, w) in &mut self.watched {
            if w.last.is_some_and(|l| now < l + MIN_GAP) || !w.dirty.swap(false, Ordering::AcqRel) {
                continue;
            }
            let Some(t) = w.term.upgrade() else { continue };
            let f = frame(&t);
            if let Some(update) = screen::diff(w.sent.as_ref(), &f) {
                let mut line = serde_json::to_vec(&Line { screen: Pushed { block_id: id, update } }).expect("json");
                line.push(b'\n');
                out.push(line);
                w.last = Some(now);
            }
            w.sent = Some(f);
        }
        out
    }

    /// When the earliest block held back by `MIN_GAP` may go.
    pub fn due(&self) -> Option<tokio::time::Instant> {
        let held = self.watched.values().filter(|w| w.dirty.load(Ordering::Acquire)).filter_map(|w| w.last);
        held.min().map(|l| tokio::time::Instant::from_std(l + MIN_GAP))
    }
}

pub(crate) fn add(r: &mut Registry) {
    // A connection's own calls go through `Screens::watch` (serve.rs); this serves help.all and in-process callers.
    r.add(
        "block.watch",
        "On a socket connection, receive these blocks' screens as `{\"screen\":{block_id, full, lines, \
         cols, rows, cursor, app_cursor, bracketed_paste}}` lines: a whole screen first (`full`), then only \
         the rows that changed. No seq: each line is a diff against the last one for that block. Replaces \
         the set watched before; `[]` stops.",
        |ctx, p: Watch| {
            terminals(ctx, &p.block_ids)?;
            Ok(Empty {})
        },
    );
}
