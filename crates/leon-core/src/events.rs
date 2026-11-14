//! The event log: what changed, in the order it changed. `State` collects events in an outbox
//! as it mutates; the server takes them while it still holds the state lock, so their order is the
//! order of the changes, and a snapshot taken under that lock sits exactly between two events.

use serde_json::{json, Value};

use crate::program_status::Change;
use crate::terminal::{Notice, NAMESPACE};
use crate::State;

#[derive(Debug, Clone)]
pub struct Event {
    /// The session it is about (for `session_destroyed`, the one destroyed).
    pub session_id: String,
    /// A server event (`session_created`, `session_destroyed`), which session-lifecycle subscribers get.
    pub server: bool,
    /// The event in the spec's CLI JSON shape (docs/spec.md §4).
    pub body: Value,
}

/// How far the outbox and the view marks stood at some point (`State::mark`).
#[derive(Debug, Clone, Copy)]
pub struct Mark {
    events: usize,
    dirty: usize,
}

impl State {
    pub fn mark(&self) -> Mark {
        Mark { events: self.outbox.len(), dirty: self.dirty.len() }
    }

    /// Forget the events and revision bumps since `mark`, for a call that failed and undid its changes.
    pub fn forget_since(&mut self, mark: Mark) {
        self.outbox.truncate(mark.events);
        for sid in self.dirty.split_off(mark.dirty) {
            if let Ok(s) = self.session_mut(&sid) {
                s.revision -= 1;
            }
        }
    }

    /// The events since the last call, with one `session_view_changed` per session whose view changed.
    pub fn take_events(&mut self) -> Vec<Event> {
        for sid in std::mem::take(&mut self.dirty) {
            if let Ok(s) = self.session(&sid) {
                let revision = s.revision;
                self.emit(&sid, "session_view_changed", json!({ "revision": revision }));
            }
        }
        std::mem::take(&mut self.outbox)
    }

    /// Mark a session's view changed: the first mark since the last `take_events` bumps its revision.
    pub(crate) fn touch(&mut self, session_id: &str) {
        if self.dirty.iter().any(|s| s == session_id) {
            return;
        }
        if let Ok(s) = self.session_mut(session_id) {
            s.revision += 1;
            self.dirty.push(session_id.to_string());
        }
    }

    pub(crate) fn emit_server(&mut self, session_id: &str, kind: &str, payload: Value) {
        let body = json!({ "type": kind, "payload": payload });
        self.outbox.push(Event { session_id: session_id.into(), server: true, body });
    }

    /// A server-wide event about no session (`keymap_changed`): only `--all` subscribers get it.
    pub fn emit_global(&mut self, kind: &str, payload: Value) {
        let body = json!({ "type": kind, "payload": payload });
        self.outbox.push(Event { session_id: String::new(), server: false, body });
    }

    pub(crate) fn emit(&mut self, session_id: &str, kind: &str, payload: Value) {
        let event = json!({ "type": kind, "payload": payload });
        let body = json!({ "type": "session_event", "payload": { "session_id": session_id, "event": event } });
        self.outbox.push(Event { session_id: session_id.into(), server: false, body });
    }

    /// A terminal event for `block_id`; dropped if the block is gone.
    pub(crate) fn emit_block(&mut self, block_id: &str, name: &str, payload: Value) {
        let Ok((s, _)) = self.block(block_id) else { return };
        let sid = s.id.clone();
        let inner = json!({ "block_id": block_id, "creator_name": NAMESPACE, "name": name, "payload": payload });
        self.emit(&sid, "block_event", inner);
    }

    /// Something a block's terminal reported.
    pub fn notice(&mut self, block_id: &str, notice: Notice) {
        let (name, payload) = match notice {
            Notice::Exited(code) => return self.block_exited(block_id, code),
            Notice::Title(title) => ("title_changed", json!({ "title": title })),
            Notice::Pwd(pwd) => ("pwd_changed", json!({ "pwd": pwd })),
            Notice::Bell => ("bell", json!({})),
            Notice::ProgramStatus(change) => {
                let name = match change {
                    Change::Changed { .. } => "program_status_changed",
                    Change::Removed { .. } => "program_status_removed",
                };
                let mut payload = serde_json::to_value(change).expect("changes serialize");
                payload.as_object_mut().map(|o| o.remove("type"));
                (name, payload)
            }
        };
        self.emit_block(block_id, name, payload);
    }
}
