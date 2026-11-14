//! The OSC 7501 program-status protocol (docs/spec.md §5): scanner, parser, per-terminal store.
//!
//! Pure: the terminal's reader feeds PTY output into a [`Tracker`], hands the pass-through bytes to
//! the emulator, writes `reply` back to the PTY, and forwards the [`Change`]s as events.

use std::collections::HashSet;

mod report;
mod scan;
mod store;

pub use report::strip_hidden;
pub use report::{parse, Ignored, Kind, Message, Record, State};
pub use scan::{Piece, Scanner};
pub use store::{Change, Reason, Store, MAX_RECORDS};

/// What a terminal answers to `OSC 7501 ; ? ST`. Constant, so report content is never echoed.
pub const QUERY_REPLY: &[u8] =
    b"\x1b]7501;?:states=idle,working,done,blocked,error:kinds=permission,question,auth\x1b\\";

/// A scanner and a store for one terminal.
#[derive(Debug, Default)]
pub struct Tracker {
    scanner: Scanner,
    pub store: Store,
}

/// The result of one [`Tracker::feed`].
#[derive(Debug, Default)]
pub struct Fed {
    pub changes: Vec<Change>,
    /// Bytes to write back to the PTY: one query reply at most, however many queries the read held.
    pub reply: Vec<u8>,
    /// The last OSC 7 payload (a `file://host/path` URL, unchecked) the read completed.
    pub pwd: Option<Vec<u8>>,
}

impl Tracker {
    /// Scans one read of PTY output; `pass` gets the bytes for the emulator, in order.
    pub fn feed(&mut self, input: &[u8], mut pass: impl FnMut(&[u8])) -> Fed {
        let mut fed = Fed::default();
        let store = &mut self.store;
        let changes = &mut fed.changes;
        self.scanner.feed(input, |piece| match piece {
            Piece::Bytes(b) => pass(b),
            Piece::Report(payload) => match parse(payload) {
                Ok(Message::Query) => {
                    if fed.reply.is_empty() {
                        fed.reply.extend_from_slice(QUERY_REPLY)
                    }
                }
                Ok(message) => changes.extend(store.apply(message)),
                Err(_) => {}
            },
            Piece::Prompt => changes.extend(store.prompt()),
            Piece::Reset => changes.extend(store.reset()),
        });
        fed.pwd = self.scanner.take_pwd();
        fed
    }
}

/// A batch of changes (one PTY read's) reduced to its net effect: each id at most once, with its last
/// change, and removals with one reason merged. So a program flapping a record makes one event per
/// read, not one per report.
pub fn coalesce(batch: Vec<Change>) -> Vec<Change> {
    let mut out = Vec::new();
    batch.into_iter().for_each(|c| add(&mut out, c));
    out
}

/// `c` replaces every earlier change to its ids; a removal joins an earlier one with the same reason.
fn add(changes: &mut Vec<Change>, c: Change) {
    let ids: HashSet<Option<String>> = match &c {
        Change::Changed { id, .. } => HashSet::from([id.clone()]),
        Change::Removed { ids, .. } => ids.iter().cloned().collect(),
    };
    changes.retain_mut(|old| match old {
        Change::Changed { id, .. } => !ids.contains(id),
        Change::Removed { ids: gone, .. } => {
            gone.retain(|g| !ids.contains(g));
            !gone.is_empty()
        }
    });
    if let Change::Removed { ids: new, reason } = &c {
        let same = changes.iter_mut().find(|o| matches!(o, Change::Removed { reason: r, .. } if r == reason));
        if let Some(Change::Removed { ids: gone, .. }) = same {
            gone.extend(new.iter().cloned());
            gone.sort();
            return;
        }
    }
    changes.push(c);
}
