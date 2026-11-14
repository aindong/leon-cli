//! Event subscriptions. A connection that attaches gets a `Subscriber`: what it asked for, its own
//! gapless seq, and a queue of encoded lines its socket task writes out. Events are queued while
//! the state lock is held and the snapshot is taken under that same lock, so a client applies the
//! snapshot, then every event with a higher seq, and misses nothing.
//!
//! A subscriber whose queue would pass `MAX_BACKLOG` is dropped and its queue freed (docs/spec.md
//! §3). Queueing never waits, so a client that stops reading slows nobody else.

use std::collections::{HashSet, VecDeque};
use std::sync::{Arc, Mutex, MutexGuard};

use leon_core::events::Event;
use leon_core::ApiError;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::sync::Notify;

use crate::methods::{session_list, view, SessionRef, SessionSummary, SessionView};
use crate::registry::parse;
use crate::{Ctx, Registry};

/// How far behind (in encoded bytes) a connection may fall before it is dropped.
pub const MAX_BACKLOG: usize = 8 << 20;

pub(crate) fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

#[derive(Deserialize, JsonSchema, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum EventScope {
    /// Session lifecycle: `session_created`, `session_destroyed`.
    Session,
    /// Everything, in every session.
    All,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AttachAll {
    pub event_scope: EventScope,
}

#[derive(Serialize, JsonSchema)]
pub struct Attached {
    /// The connection's event seq at this snapshot; every later event on it has a higher one.
    pub seq: u64,
    pub view: SessionView,
}

#[derive(Serialize, JsonSchema)]
pub struct AttachedAll {
    /// As in `session.attach`.
    pub seq: u64,
    pub sessions: Vec<SessionSummary>,
}

#[derive(Default)]
struct Queue {
    all: bool,
    lifecycle: bool,
    sessions: HashSet<String>,
    seq: u64,
    lines: VecDeque<Vec<u8>>,
    bytes: usize,
    dropped: bool,
    registered: bool,
}

impl Queue {
    fn wants(&self, e: &Event) -> bool {
        self.all || (e.server && self.lifecycle) || e.session_id.as_ref().is_some_and(|s| self.sessions.contains(s))
    }
}

/// The subscriber was dropped for falling too far behind.
#[derive(Debug)]
pub struct Dropped;

#[derive(Default)]
pub struct Subscriber {
    queue: Mutex<Queue>,
    /// Woken when a line is queued or the subscriber is dropped.
    pub ready: Notify,
}

impl Subscriber {
    /// Queue `e` as `{"event":…,"seq":N}` if it is in scope. Called with the state lock held.
    pub(crate) fn offer(&self, e: &Event) {
        let mut q = lock(&self.queue);
        if q.dropped || !q.wants(e) {
            return;
        }
        q.seq += 1;
        let mut line = serde_json::to_vec(&json!({ "event": e.body, "seq": q.seq })).expect("events serialize");
        line.push(b'\n');
        q.bytes += line.len();
        if q.bytes > MAX_BACKLOG {
            q.dropped = true;
            q.lines = VecDeque::new();
        } else {
            q.lines.push_back(line);
        }
        drop(q);
        self.ready.notify_one();
    }

    /// The next queued line, if any.
    pub fn pop(&self) -> Result<Option<Vec<u8>>, Dropped> {
        let mut q = lock(&self.queue);
        if q.dropped {
            return Err(Dropped);
        }
        let line = q.lines.pop_front();
        q.bytes -= line.as_ref().map_or(0, Vec::len);
        Ok(line)
    }

    pub fn is_dropped(&self) -> bool {
        lock(&self.queue).dropped
    }
}

impl Ctx {
    /// Route an attach call from a connection to `sub`; the socket loop uses this instead of the registry.
    pub fn attach(&self, sub: &Arc<Subscriber>, method: &str, params: Value) -> Result<Value, ApiError> {
        let r = match method {
            "session.attach" => serde_json::to_value(self.attach_session(sub, parse(method, params)?)?),
            _ => serde_json::to_value(self.attach_all(sub, parse(method, params)?)?),
        };
        Ok(r.expect("snapshots serialize"))
    }

    /// Snapshot one session, and have `sub` get its events from here on.
    pub fn attach_session(&self, sub: &Arc<Subscriber>, p: SessionRef) -> Result<Attached, ApiError> {
        let st = self.state();
        let view = view(st.session(&p.session_id)?);
        let seq = self.widen(sub, |q| {
            q.sessions.insert(p.session_id);
        });
        Ok(Attached { seq, view })
    }

    /// List the sessions, and have `sub` get the events of `event_scope` from here on.
    pub fn attach_all(&self, sub: &Arc<Subscriber>, p: AttachAll) -> Result<AttachedAll, ApiError> {
        let st = self.state();
        let sessions = session_list(&st);
        let seq = self.widen(sub, |q| match p.event_scope {
            EventScope::Session => q.lifecycle = true,
            EventScope::All => q.all = true,
        });
        Ok(AttachedAll { seq, sessions })
    }

    /// Change what `sub` gets and register it. Call with the state lock held. Returns its seq.
    fn widen(&self, sub: &Arc<Subscriber>, f: impl FnOnce(&mut Queue)) -> u64 {
        let mut q = lock(&sub.queue);
        f(&mut q);
        let (seq, first) = (q.seq, !std::mem::replace(&mut q.registered, true));
        drop(q);
        if first {
            lock(&self.subscribers).push(Arc::downgrade(sub));
        }
        seq
    }
}

/// Whether the socket loop must route `method` through `Ctx::attach`.
pub fn is_attach(method: &str) -> bool {
    matches!(method, "session.attach" | "session.attach_all")
}

pub(crate) fn add(r: &mut Registry) {
    // A connection's own calls go through `Ctx::attach` (serve.rs); these serve help.all and in-process callers.
    r.add(
        "session.attach",
        "Snapshot a session; on a socket connection, also receive its events (block events too) as \
         `{\"event\":…,\"seq\":N}` lines from now on, with seq above the snapshot's.",
        |ctx, p: SessionRef| ctx.attach_session(&Arc::default(), p),
    );
    r.add(
        "session.attach_all",
        "List sessions; on a socket connection, also receive session lifecycle events (event_scope \
         `session`) or every event (`all`) from now on.",
        |ctx, p: AttachAll| ctx.attach_all(&Arc::default(), p),
    );
}
