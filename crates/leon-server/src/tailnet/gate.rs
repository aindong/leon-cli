//! The tailnet listener's gate (docs/architecture/tailnet.md, "Limits"): refusals counted per
//! address, and connections waiting for a decision capped, in all and per address.

use std::collections::HashMap;
use std::net::IpAddr;
use std::time::{Duration, Instant};

/// Refusals an address may have in `WINDOW` before its connections are closed unasked.
const FAILS: u32 = 5;
pub(super) const WINDOW: Duration = Duration::from_secs(60);
/// Addresses the refusal count remembers; when full, a new address is refused unasked.
const MOST_ADDRS: usize = 4096;
/// Connections waiting for a decision, in all and from one address.
const PENDING: usize = 32;
// a browser opens up to 6 connections to load one page, each decided on its own
const PENDING_PER: usize = 8;

/// What a decision means for an address's refusal count.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(super) enum Outcome {
    /// Let in: the count is cleared.
    In,
    /// The caller was refused: it counts.
    Refused,
    /// tailscaled failed, not the caller: the count stays as it is.
    Unsure,
}

/// Refusal counts and connections waiting, by address.
#[derive(Default)]
pub(super) struct Gate {
    /// Refusals since a time, and whether the limit was logged.
    fails: HashMap<IpAddr, (u32, Instant, bool)>,
    pending: HashMap<IpAddr, usize>,
}

pub(super) enum Turned {
    /// Too many refusals lately; `first` when this is the first connection turned for it.
    Limited {
        first: bool,
    },
    Busy,
}

impl Gate {
    pub(super) fn enter(&mut self, ip: IpAddr) -> Result<(), Turned> {
        if self.fails.len() >= MOST_ADDRS {
            self.fails.retain(|_, f| f.1.elapsed() < WINDOW);
        }
        let full = self.fails.len() >= MOST_ADDRS;
        match self.fails.get_mut(&ip) {
            Some(f) if f.1.elapsed() >= WINDOW => {
                self.fails.remove(&ip);
            }
            Some(f) if f.0 >= FAILS => return Err(Turned::Limited { first: !std::mem::replace(&mut f.2, true) }),
            Some(_) => {}
            None if full => return Err(Turned::Limited { first: false }),
            None => {}
        }
        let mine = self.pending.get(&ip).copied().unwrap_or(0);
        if mine >= PENDING_PER || self.pending.values().sum::<usize>() >= PENDING {
            return Err(Turned::Busy);
        }
        self.pending.insert(ip, mine + 1);
        Ok(())
    }

    /// The decision is made: a refusal counts against the address, a let-in clears its count.
    pub(super) fn leave(&mut self, ip: IpAddr, outcome: Outcome) {
        match self.pending.get_mut(&ip) {
            Some(n) if *n > 1 => *n -= 1,
            _ => {
                self.pending.remove(&ip);
            }
        }
        match outcome {
            Outcome::Refused => self.fails.entry(ip).or_insert((0, Instant::now(), false)).0 += 1,
            Outcome::In => {
                self.fails.remove(&ip);
            }
            Outcome::Unsure => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_gate_limits_refusals_and_waiting() {
        let mut g = Gate::default();
        let ip: IpAddr = "100.64.0.9".parse().unwrap();
        for _ in 0..FAILS {
            assert!(g.enter(ip).is_ok());
            g.leave(ip, Outcome::Refused);
        }
        assert!(matches!(g.enter(ip), Err(Turned::Limited { first: true })));
        assert!(matches!(g.enter(ip), Err(Turned::Limited { first: false })), "logged once");
        let other: IpAddr = "100.64.0.10".parse().unwrap();
        for _ in 0..PENDING_PER {
            assert!(g.enter(other).is_ok());
        }
        assert!(matches!(g.enter(other), Err(Turned::Busy)), "per address");
        g.leave(other, Outcome::Refused);
        g.leave(other, Outcome::Unsure);
        assert_eq!(g.fails[&other].0, 1, "tailscaled failing doesn't clear or add to a count");
        assert!(g.enter(other).is_ok());
        g.leave(other, Outcome::In);
        assert!(!g.fails.contains_key(&other), "a let-in clears its count");
        let mut full = Gate::default();
        for i in 0..MOST_ADDRS as u32 {
            full.fails.insert(IpAddr::from((0x6440_0000 + i).to_be_bytes()), (1, Instant::now(), false));
        }
        assert!(matches!(full.enter("100.127.0.1".parse().unwrap()), Err(Turned::Limited { .. })), "full: refused");
    }
}
