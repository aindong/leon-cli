//! Resumable event streams (docs/architecture/hosts.md, "Remote hosts"). A subscriber attached with
//! `resumable` has a token. When its connection ends, it is parked for `HOLD`, still queueing
//! events (and still dropped past `MAX_BACKLOG`). `session.resume {resume, seq}` on another
//! connection moves its queue there under the state lock, so no event slips between, and puts the
//! kept lines after `seq` back in front: the client gets every event after the last it saw, once.
//! A resume while the old connection is still open (a link that died without closing) takes the
//! stream over, and the old connection ends.

use std::collections::HashMap;
use std::sync::{Arc, Weak};
use std::time::{Duration, Instant};

use leon_core::ApiError;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::events::{lock, Subscriber};
use crate::{Ctx, Registry};

/// How long a dropped stream waits for its client.
pub const HOLD: Duration = Duration::from_secs(60);

/// How long a client retries a dropped link: `HOLD`, or `LEON_LINK_HOLD_SECS` (tests).
pub fn link_hold() -> Duration {
    leon_core::secs_from_env("LEON_LINK_HOLD_SECS", HOLD)
}
/// How many bytes of written lines a resumable stream keeps.
pub const KEEP: usize = 1 << 20;

pub(crate) enum Slot {
    /// On a connection.
    Live(Weak<Subscriber>),
    /// Its connection ended at this time.
    Parked(Arc<Subscriber>, Instant),
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Resume {
    /// The token `session.attach` or `session.attach_all` returned with `resumable`.
    pub resume: String,
    /// The last seq the client got; events come again from the next one.
    pub seq: u64,
}

#[derive(Serialize, JsonSchema)]
pub struct Resumed {
    /// The seq resumed after.
    pub seq: u64,
}

fn failed(why: &str) -> ApiError {
    ApiError::new(410, "resume_failed", format!("can't resume: {why}; attach again"))
}

/// Forget streams that ended, waited too long, or fell too far behind. Takes queue locks, so
/// call it with the slots locked and no queue lock held.
fn sweep(slots: &mut HashMap<String, Slot>) {
    slots.retain(|_, s| match s {
        Slot::Live(w) => w.upgrade().is_some_and(|s| !s.is_dropped()),
        Slot::Parked(s, at) => at.elapsed() < HOLD && !s.is_dropped(),
    });
}

impl Ctx {
    /// Give `sub` a token (once) and return it. Locks: slots, then the queue.
    pub(crate) fn make_resumable(&self, sub: &Arc<Subscriber>) -> String {
        let mut slots = lock(&self.resumable);
        sweep(&mut slots);
        let token = lock(&sub.queue).resume.get_or_insert_with(|| leon_core::new_id("resume")).clone();
        slots.insert(token.clone(), Slot::Live(Arc::downgrade(sub)));
        token
    }

    /// A connection ended: keep its resumable stream for `HOLD`, unless it was taken over.
    pub fn park(&self, sub: &Arc<Subscriber>) {
        let mut slots = lock(&self.resumable);
        let q = lock(&sub.queue);
        let Some(token) = q.resume.clone().filter(|_| !q.dropped) else { return };
        drop(q);
        slots.insert(token, Slot::Parked(sub.clone(), Instant::now()));
    }

    /// Drop parked streams past `HOLD` (the socket loop's tick).
    pub fn sweep_parked(&self) {
        sweep(&mut lock(&self.resumable));
    }

    /// Move the stream named by `p.resume` to `sub`, this connection's own subscriber.
    pub fn resume(&self, sub: &Arc<Subscriber>, p: Resume) -> Result<Resumed, ApiError> {
        let _st = self.state(); // no event is offered while the queue moves
        let mut slots = lock(&self.resumable);
        sweep(&mut slots);
        if lock(&sub.queue).registered {
            return Err(failed("this connection already attached"));
        }
        let old = match slots.get(&p.resume) {
            Some(Slot::Live(w)) => w.upgrade(),
            Some(Slot::Parked(s, _)) => Some(s.clone()),
            None => None,
        }
        .ok_or_else(|| failed("no such stream (unknown, expired, or the server restarted)"))?;
        let mut oq = lock(&old.queue);
        if p.seq > oq.seq {
            return Err(failed("that seq is ahead of the stream"));
        }
        let first = oq.kept.front().or(oq.lines.front()).map_or(oq.seq + 1, |(s, _)| *s);
        if p.seq + 1 < first {
            return Err(failed("events after that seq are no longer kept"));
        }
        let mut q = std::mem::take(&mut *oq);
        oq.dropped = true; // the old connection, if still open, stops and ends
        drop(oq);
        old.ready.notify_one();
        for (seq, line) in std::mem::take(&mut q.kept).into_iter().rev().filter(|(s, _)| *s > p.seq) {
            q.bytes += line.len();
            q.lines.push_front((seq, line));
        }
        q.kept_bytes = 0;
        let token = q.resume.clone().expect("a resumable stream has a token");
        *lock(&sub.queue) = q;
        slots.insert(token, Slot::Live(Arc::downgrade(sub)));
        drop(slots);
        self.register(sub);
        sub.ready.notify_one();
        Ok(Resumed { seq: p.seq })
    }
}

pub(crate) fn add(r: &mut Registry) {
    r.add(
        "session.resume",
        "On a socket connection that hasn't attached: take over a resumable event stream (see \
         `session.attach_all`) and get its events after `seq` again, then the new ones, with the \
         same seq counter. Fails with `resume_failed` when the stream is gone or `seq` is no \
         longer kept; then attach again.",
        |_ctx, _p: Resume| -> Result<Resumed, ApiError> { Err(failed("only on a socket connection")) },
    );
}
