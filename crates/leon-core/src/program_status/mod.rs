//! The OSC 7501 program-status protocol (docs/spec.md §5): scanner, parser, per-terminal store.
//!
//! Pure: M2 feeds PTY output into a [`Tracker`] per terminal, hands the pass-through bytes to the
//! emulator, writes `reply` back to the PTY, and M3 forwards the [`Change`]s as events.

mod report;
mod scan;
mod store;

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
    /// Bytes to write back to the PTY (query replies); usually empty.
    pub reply: Vec<u8>,
}

impl Tracker {
    /// Scans one read of PTY output; `pass` gets the bytes for the emulator, in order.
    pub fn feed(&mut self, input: &[u8], mut pass: impl FnMut(&[u8])) -> Fed {
        let mut fed = Fed::default();
        let store = &mut self.store;
        self.scanner.feed(input, |piece| match piece {
            Piece::Bytes(b) => pass(b),
            Piece::Report(payload) => match parse(payload) {
                Ok(Message::Query) => fed.reply.extend_from_slice(QUERY_REPLY),
                Ok(message) => fed.changes.extend(store.apply(message)),
                Err(_) => {}
            },
            Piece::Prompt => fed.changes.extend(store.prompt()),
            Piece::Reset => fed.changes.extend(store.reset()),
        });
        fed
    }
}
