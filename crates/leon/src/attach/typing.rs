//! Keys typed over a host's link arrive whole or not at all (daily-driver F12). A write sent into
//! a stalled link may land long after, over the old ssh connection, behind anything sent on the
//! new one, so a key goes out only while the calls connection answered within the last second;
//! otherwise a probe goes first and the keys wait for its answer. A failed probe or write drops
//! every key waiting, and so does a key typed while the link is reconnecting; the bar says how many
//! weren't sent. A paste over a link waits in the same line, so it can't pass keys typed before it.
//! Left open: a stall that starts within that second can catch one write in flight.

use std::time::{Duration, Instant};

/// A reply this recent shows the link is live, so keys go out without a probe.
const FRESH: Duration = Duration::from_secs(1);
/// Bytes of keys that may wait behind a probe or a write.
const MAX_WAITING: usize = 4 << 10;
/// Bytes of keys and pastes that may wait (attach refuses a paste over 400 KiB before it gets here).
const MAX_WAITING_PASTES: usize = 512 << 10;

#[derive(Default)]
pub(super) struct Typing {
    /// The last answer on the calls connection.
    heard: Option<Instant>,
    /// A probe or a write is out; keys wait behind it.
    busy: bool,
    /// Keys were dropped while one was out: drop the rest until it answers, or they would go
    /// without the keys before them.
    held: bool,
    /// Keys and pastes waiting, oldest first.
    waiting: Vec<Waiting>,
    /// Keys dropped since a write last went through, for the bar.
    pub dropped: usize,
}

struct Waiting {
    sid: String,
    block: String,
    bytes: Vec<u8>,
    paste: bool,
}

/// What to send now.
#[derive(Debug, PartialEq)]
pub(super) enum Out {
    Nothing,
    /// `server.version`, to learn whether the link is live before any key goes.
    Probe,
    /// One write of the keys typed into one block, in order.
    Write(String, String, Vec<u8>),
    /// A paste into one block (session, block, text).
    Paste(String, String, String),
}

impl Typing {
    /// A key typed into `block`: dropped while the link is down, else queued; returns what to send.
    pub fn key(&mut self, sid: &str, block: &str, bytes: Vec<u8>, link_up: bool, now: Instant) -> Out {
        self.queue(Waiting { sid: sid.into(), block: block.into(), bytes, paste: false }, link_up, now)
    }

    /// A paste into `block`, in line with the keys.
    pub fn paste(&mut self, sid: &str, block: &str, text: &str, link_up: bool, now: Instant) -> Out {
        let w = Waiting { sid: sid.into(), block: block.into(), bytes: text.as_bytes().to_vec(), paste: true };
        self.queue(w, link_up, now)
    }

    fn queue(&mut self, w: Waiting, link_up: bool, now: Instant) -> Out {
        let bytes =
            |paste: bool| self.waiting.iter().filter(|q| paste || !q.paste).map(|q| q.bytes.len()).sum::<usize>();
        let full = match w.paste {
            false => bytes(false) + w.bytes.len() > MAX_WAITING,
            true => bytes(true) + w.bytes.len() > MAX_WAITING_PASTES,
        };
        if self.held || !link_up || full {
            self.drop_waiting();
            self.dropped += 1;
            self.held = self.busy;
            return Out::Nothing;
        }
        self.waiting.push(w);
        self.next(now)
    }

    /// The probe or write came back.
    pub fn answered(&mut self, ok: bool, now: Instant) -> Out {
        (self.busy, self.held) = (false, false);
        if !ok {
            self.heard = None;
            self.drop_waiting();
            return Out::Nothing;
        }
        self.heard = Some(now);
        self.next(now)
    }

    /// "3 keys not sent", while any are.
    pub fn not_sent(&self) -> Option<String> {
        match self.dropped {
            0 => None,
            1 => Some("1 key not sent".into()),
            n => Some(format!("{n} keys not sent")),
        }
    }

    fn drop_waiting(&mut self) {
        self.dropped += self.waiting.len();
        self.waiting.clear();
    }

    fn next(&mut self, now: Instant) -> Out {
        if self.busy || self.waiting.is_empty() {
            return Out::Nothing;
        }
        self.busy = true;
        if !self.heard.is_some_and(|h| now.duration_since(h) < FRESH) {
            return Out::Probe;
        }
        self.dropped = 0;
        if self.waiting[0].paste {
            let w = self.waiting.remove(0);
            return Out::Paste(w.sid, w.block, String::from_utf8_lossy(&w.bytes).into_owned());
        }
        let first = &self.waiting[0];
        let (sid, block) = (first.sid.clone(), first.block.clone());
        let n = self.waiting.iter().take_while(|q| q.sid == sid && q.block == block && !q.paste).count();
        let bytes = self.waiting.drain(..n).flat_map(|q| q.bytes).collect();
        Out::Write(sid, block, bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w(block: &str, bytes: &str) -> Out {
        Out::Write("s".into(), block.into(), bytes.as_bytes().to_vec())
    }

    #[test]
    fn after_a_quiet_spell_keys_wait_for_a_probe_then_go_in_one_write() {
        let mut t = Typing::default();
        let now = Instant::now();
        assert_eq!(t.key("s", "b", b"e".to_vec(), true, now), Out::Probe);
        assert_eq!(t.key("s", "b", b"c".to_vec(), true, now), Out::Nothing);
        assert_eq!(t.answered(true, now), w("b", "ec"));
        // a live link: the next key goes behind the write, then without a probe
        assert_eq!(t.key("s", "b", b"h".to_vec(), true, now), Out::Nothing);
        assert_eq!(t.answered(true, now), w("b", "h"));
        assert_eq!(t.answered(true, now), Out::Nothing);
        assert_eq!(t.key("s", "b", b"o".to_vec(), true, now), w("b", "o"));
        assert_eq!(t.answered(true, now + FRESH), Out::Nothing);
        assert_eq!(t.key("s", "b", b"!".to_vec(), true, now + FRESH * 2), Out::Probe, "quiet again");
    }

    #[test]
    fn a_failed_probe_drops_every_key_waiting() {
        let mut t = Typing::default();
        let now = Instant::now();
        assert_eq!(t.key("s", "b", b"e".to_vec(), true, now), Out::Probe);
        for k in "cho during-pause\r".bytes() {
            assert_eq!(t.key("s", "b", vec![k], true, now), Out::Nothing);
        }
        assert_eq!(t.answered(false, now), Out::Nothing);
        assert_eq!(t.dropped, 18);
        // while the link is reconnecting, keys are dropped at once
        assert_eq!(t.key("s", "b", b"x".to_vec(), false, now), Out::Nothing);
        assert_eq!(t.dropped, 19);
        // back up: a probe first, and the count goes once a write goes through
        assert_eq!(t.key("s", "b", b"y".to_vec(), true, now), Out::Probe);
        assert_eq!(t.answered(true, now), w("b", "y"));
        assert_eq!(t.dropped, 0);
    }

    #[test]
    fn keys_for_another_block_wait_for_the_next_write_and_too_many_are_dropped() {
        let mut t = Typing::default();
        let now = Instant::now();
        assert_eq!(t.key("s", "a", b"1".to_vec(), true, now), Out::Probe);
        t.key("s", "b", b"2".to_vec(), true, now);
        assert_eq!(t.answered(true, now), w("a", "1"));
        assert_eq!(t.answered(true, now), w("b", "2"));
        t.answered(true, now);
        t.key("s", "a", vec![b'x'; MAX_WAITING], true, now); // out as one write
        assert_eq!(t.key("s", "a", b"y".to_vec(), true, now), Out::Nothing);
        assert_eq!(t.key("s", "a", vec![b'z'; MAX_WAITING], true, now), Out::Nothing);
        assert_eq!(t.dropped, 2, "the waiting key and the one that didn't fit");
        // what follows a drop waits for nothing: it would go out without the keys before it
        assert_eq!(t.key("s", "a", b"!".to_vec(), true, now), Out::Nothing);
        assert_eq!(t.answered(true, now), Out::Nothing, "the write ahead answered; nothing was waiting");
        assert_eq!(t.dropped, 3);
        assert_eq!(t.key("s", "a", b"k".to_vec(), true, now), w("a", "k"), "after the answer, keys go again");
    }

    #[test]
    fn a_paste_waits_in_line_with_the_keys() {
        let mut t = Typing::default();
        let now = Instant::now();
        assert_eq!(t.key("s", "b", b"cd /tmp && ".to_vec(), true, now), Out::Probe);
        assert_eq!(t.paste("s", "b", &"x".repeat(MAX_WAITING * 2), true, now), Out::Nothing, "a big paste fits");
        assert_eq!(t.key("s", "b", b"\r".to_vec(), true, now), Out::Nothing);
        assert_eq!(t.answered(true, now), w("b", "cd /tmp && "));
        assert_eq!(t.answered(true, now), Out::Paste("s".into(), "b".into(), "x".repeat(MAX_WAITING * 2)));
        assert_eq!(t.answered(true, now), w("b", "\r"));
    }
}
