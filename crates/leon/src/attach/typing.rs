//! Keys typed over a host's link arrive whole or not at all (daily-driver F12). A write sent into
//! a stalled link may land long after, over the old ssh connection, behind anything sent on the
//! new one, so a key goes out only while the calls connection answered within the last second;
//! otherwise a probe goes first and the keys wait for its answer. A failed probe or write drops
//! every key waiting, and so does a key typed while the link is reconnecting; the bar says how many
//! weren't sent. Left open: a stall that starts within that second can catch one write in flight.

use std::time::{Duration, Instant};

/// A reply this recent shows the link is live, so keys go out without a probe.
const FRESH: Duration = Duration::from_secs(1);
/// Bytes of keys that may wait behind a probe or a write (a paste goes its own way).
const MAX_WAITING: usize = 4 << 10;

#[derive(Default)]
pub(super) struct Typing {
    /// The last answer on the calls connection.
    heard: Option<Instant>,
    /// A probe or a write is out; keys wait behind it.
    busy: bool,
    /// Keys waiting, oldest first: (session, block, bytes).
    waiting: Vec<(String, String, Vec<u8>)>,
    /// Keys dropped since a write last went through, for the bar.
    pub dropped: usize,
}

/// What to send now.
#[derive(Debug, PartialEq)]
pub(super) enum Send {
    Nothing,
    /// `server.version`, to learn whether the link is live before any key goes.
    Probe,
    /// One write of the keys typed into one block, in order.
    Write(String, String, Vec<u8>),
}

impl Typing {
    /// A key typed into `block`: dropped while the link is down, else queued; returns what to send.
    pub fn key(&mut self, sid: &str, block: &str, bytes: Vec<u8>, link_up: bool, now: Instant) -> Send {
        let queued: usize = self.waiting.iter().map(|(_, _, b)| b.len()).sum();
        if !link_up || queued + bytes.len() > MAX_WAITING {
            self.drop_waiting();
            self.dropped += 1;
            return Send::Nothing;
        }
        self.waiting.push((sid.to_string(), block.to_string(), bytes));
        self.next(now)
    }

    /// The probe or write came back.
    pub fn answered(&mut self, ok: bool, now: Instant) -> Send {
        self.busy = false;
        if !ok {
            self.heard = None;
            self.drop_waiting();
            return Send::Nothing;
        }
        self.heard = Some(now);
        self.next(now)
    }

    fn drop_waiting(&mut self) {
        self.dropped += self.waiting.len();
        self.waiting.clear();
    }

    fn next(&mut self, now: Instant) -> Send {
        if self.busy || self.waiting.is_empty() {
            return Send::Nothing;
        }
        self.busy = true;
        if !self.heard.is_some_and(|h| now.duration_since(h) < FRESH) {
            return Send::Probe;
        }
        let (sid, block) = (self.waiting[0].0.clone(), self.waiting[0].1.clone());
        let n = self.waiting.iter().take_while(|(s, b, _)| *s == sid && *b == block).count();
        let bytes = self.waiting.drain(..n).flat_map(|(_, _, b)| b).collect();
        self.dropped = 0;
        Send::Write(sid, block, bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w(block: &str, bytes: &str) -> Send {
        Send::Write("s".into(), block.into(), bytes.as_bytes().to_vec())
    }

    #[test]
    fn after_a_quiet_spell_keys_wait_for_a_probe_then_go_in_one_write() {
        let mut t = Typing::default();
        let now = Instant::now();
        assert_eq!(t.key("s", "b", b"e".to_vec(), true, now), Send::Probe);
        assert_eq!(t.key("s", "b", b"c".to_vec(), true, now), Send::Nothing);
        assert_eq!(t.answered(true, now), w("b", "ec"));
        // a live link: the next key goes behind the write, then without a probe
        assert_eq!(t.key("s", "b", b"h".to_vec(), true, now), Send::Nothing);
        assert_eq!(t.answered(true, now), w("b", "h"));
        assert_eq!(t.answered(true, now), Send::Nothing);
        assert_eq!(t.key("s", "b", b"o".to_vec(), true, now), w("b", "o"));
        assert_eq!(t.answered(true, now + FRESH), Send::Nothing);
        assert_eq!(t.key("s", "b", b"!".to_vec(), true, now + FRESH * 2), Send::Probe, "quiet again");
    }

    #[test]
    fn a_failed_probe_drops_every_key_waiting() {
        let mut t = Typing::default();
        let now = Instant::now();
        assert_eq!(t.key("s", "b", b"e".to_vec(), true, now), Send::Probe);
        for k in "cho during-pause\r".bytes() {
            assert_eq!(t.key("s", "b", vec![k], true, now), Send::Nothing);
        }
        assert_eq!(t.answered(false, now), Send::Nothing);
        assert_eq!(t.dropped, 18);
        // while the link is reconnecting, keys are dropped at once
        assert_eq!(t.key("s", "b", b"x".to_vec(), false, now), Send::Nothing);
        assert_eq!(t.dropped, 19);
        // back up: a probe first, and the count goes once a write goes through
        assert_eq!(t.key("s", "b", b"y".to_vec(), true, now), Send::Probe);
        assert_eq!(t.answered(true, now), w("b", "y"));
        assert_eq!(t.dropped, 0);
    }

    #[test]
    fn keys_for_another_block_wait_for_the_next_write_and_too_many_are_dropped() {
        let mut t = Typing::default();
        let now = Instant::now();
        assert_eq!(t.key("s", "a", b"1".to_vec(), true, now), Send::Probe);
        t.key("s", "b", b"2".to_vec(), true, now);
        assert_eq!(t.answered(true, now), w("a", "1"));
        assert_eq!(t.answered(true, now), w("b", "2"));
        t.answered(true, now);
        t.key("s", "a", vec![b'x'; MAX_WAITING], true, now); // out as one write
        assert_eq!(t.key("s", "a", b"y".to_vec(), true, now), Send::Nothing);
        assert_eq!(t.key("s", "a", vec![b'z'; MAX_WAITING], true, now), Send::Nothing);
        assert_eq!(t.dropped, 2, "the waiting key and the one that didn't fit");
    }
}
