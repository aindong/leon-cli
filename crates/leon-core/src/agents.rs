//! Agents started by other agents (docs/agents.md, "Orchestrate other agents"): which block works
//! for which, the reports waiting for each orchestrator, and the state each block's program last
//! reported, which looking at a pane doesn't clear.

use std::collections::{HashMap, VecDeque};
use std::time::Instant;

use serde::{Deserialize, Serialize};

use crate::model::State;
use crate::program_status::{Kind, Record, State as Status};

/// Unread reports an orchestrator keeps; past it the oldest goes.
pub const MAX_UNREAD: usize = 256;

/// A block started by another (`agent.link`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Worker {
    /// The orchestrator's block.
    pub parent: String,
    /// The agent's program name, e.g. `claude`.
    pub agent: String,
    /// Its label when linked, for reports made after the block is gone.
    pub label: Option<String>,
    /// Seconds since the epoch.
    pub started_at: u64,
}

/// What a worker told its orchestrator.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Report {
    pub block_id: String,
    pub label: Option<String>,
    pub agent: String,
    /// `done`, `blocked` or `error` as the worker reported it; `exited` or `closed` when it ended.
    pub state: String,
    pub kind: Option<Kind>,
    pub title: Option<String>,
    pub body: Option<String>,
    /// Seconds since the epoch.
    pub at: u64,
}

#[derive(Debug, Default)]
pub struct Inbox {
    pub unread: VecDeque<Report>,
    /// A report came since the last notice was typed or the inbox read.
    pub owed: bool,
}

#[derive(Debug, Default)]
pub struct Agents {
    /// By worker block.
    pub workers: HashMap<String, Worker>,
    /// By orchestrator block.
    pub inboxes: HashMap<String, Inbox>,
    /// The root record each block's program last reported (state and kind), and when.
    last: HashMap<String, (Status, Option<Kind>, Instant)>,
}

pub fn now_secs() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

impl Agents {
    /// The state block `id`'s program last reported for its root record, and when.
    pub fn last(&self, id: &str) -> Option<(Status, Option<Kind>, Instant)> {
        self.last.get(id).copied()
    }

    /// A report for `parent`: dropped when it says nothing new (no body, and the worker's newest
    /// unread report has the same state and kind), else kept, the oldest going past the cap.
    pub fn push(&mut self, parent: &str, r: Report) {
        let inbox = self.inboxes.entry(parent.to_string()).or_default();
        let newest = inbox.unread.iter().rev().find(|o| o.block_id == r.block_id);
        if r.body.is_none() && newest.is_some_and(|o| o.state == r.state && o.kind == r.kind) {
            return;
        }
        if inbox.unread.len() == MAX_UNREAD {
            inbox.unread.pop_front();
        }
        inbox.unread.push_back(r);
        inbox.owed = true;
    }

    /// `parent`'s unread reports, oldest first; unless `peek`, they are read now.
    pub fn take(&mut self, parent: &str, peek: bool) -> Vec<Report> {
        let Some(inbox) = self.inboxes.get_mut(parent) else { return vec![] };
        if peek {
            return inbox.unread.iter().cloned().collect();
        }
        inbox.owed = false;
        inbox.unread.drain(..).collect()
    }

    /// How many unread reports `parent` holds from `worker`.
    pub fn unread_from(&self, parent: &str, worker: &str) -> usize {
        self.inboxes.get(parent).map_or(0, |i| i.unread.iter().filter(|r| r.block_id == worker).count())
    }
}

impl State {
    /// Block `id`'s program reported `record` for its root: remember it, and if `id` works for
    /// another block and needs it (done, blocked, error), report to that block.
    pub fn agent_reported(&mut self, id: &str, record: &Record) {
        self.agents.last.insert(id.to_string(), (record.state, record.kind, Instant::now()));
        let Some(w) = self.agents.workers.get(id) else { return };
        let state = match record.state {
            Status::Done => "done",
            Status::Blocked => "blocked",
            Status::Error => "error",
            Status::Working | Status::Idle => return,
        };
        let label = self.block(id).ok().and_then(|(_, b)| b.label.clone()).or_else(|| w.label.clone());
        let report = Report {
            block_id: id.to_string(),
            label,
            agent: w.agent.clone(),
            state: state.into(),
            kind: record.kind,
            title: record.title.clone(),
            body: record.msg.clone(),
            at: now_secs(),
        };
        let parent = w.parent.clone();
        self.agents.push(&parent, report);
    }

    /// Settle what blocks that went or whose program ended leave behind: a worker that exited or
    /// closed reports so and leaves its team; an orchestrator that closed loses its inbox and its
    /// workers' links; a closed block's last state goes. Cheap: call it often.
    pub fn prune_agents(&mut self) {
        let alive = |st: &State, id: &str| st.block(id).ok().map(|(_, b)| b.exit_code);
        let mut ended = vec![];
        for (id, w) in &self.agents.workers {
            if alive(self, &w.parent).is_none() {
                ended.push((id.clone(), None));
                continue;
            }
            match alive(self, id) {
                Some(None) => {}
                Some(Some(code)) => ended.push((id.clone(), Some(("exited", Some(code))))),
                None => ended.push((id.clone(), Some(("closed", self.exit_status(id))))),
            }
        }
        for (id, how) in ended {
            let w = self.agents.workers.remove(&id).expect("listed above");
            let Some((state, code)) = how else { continue }; // its orchestrator is gone
            let state = if code.is_some() { "exited" } else { state };
            let report = Report {
                block_id: id.clone(),
                label: self.block(&id).ok().and_then(|(_, b)| b.label.clone()).or(w.label),
                agent: w.agent,
                state: state.into(),
                kind: None,
                title: None,
                body: code.map(|c| format!("exit status {c}")),
                at: now_secs(),
            };
            self.agents.push(&w.parent, report);
        }
        let gone: Vec<String> = self
            .agents
            .inboxes
            .keys()
            .chain(self.agents.last.keys())
            .filter(|id| self.block(id).is_err())
            .cloned()
            .collect();
        for id in gone {
            self.agents.inboxes.remove(&id);
            self.agents.last.remove(&id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report(block: &str, state: &str, body: Option<&str>) -> Report {
        Report {
            block_id: block.into(),
            label: None,
            agent: "claude".into(),
            state: state.into(),
            kind: None,
            title: None,
            body: body.map(String::from),
            at: 0,
        }
    }

    #[test]
    fn a_bodiless_repeat_adds_nothing_and_a_new_body_does() {
        let mut a = Agents::default();
        a.push("p", report("w", "done", Some("result")));
        a.push("p", report("w", "done", None));
        a.push("p", report("other", "done", None));
        a.push("p", report("w", "done", Some("second turn")));
        a.push("p", report("w", "error", None));
        let bodies: Vec<_> = a.take("p", true).into_iter().map(|r| (r.block_id, r.state, r.body)).collect();
        assert_eq!(
            bodies,
            [
                ("w".into(), "done".into(), Some("result".into())),
                ("other".into(), "done".into(), None),
                ("w".into(), "done".into(), Some("second turn".into())),
                ("w".into(), "error".into(), None),
            ]
        );
        assert!(a.inboxes["p"].owed);
        assert_eq!(a.unread_from("p", "w"), 3);
        assert_eq!(a.take("p", false).len(), 4);
        assert!(!a.inboxes["p"].owed, "read: no notice owed");
        assert!(a.take("p", false).is_empty());
        a.push("p", report("w", "done", None));
        assert_eq!(a.take("p", false).len(), 1, "a read report doesn't hide the next one");
    }

    #[test]
    fn the_oldest_goes_past_the_cap() {
        let mut a = Agents::default();
        for i in 0..MAX_UNREAD + 3 {
            a.push("p", report("w", "done", Some(&i.to_string())));
        }
        let r = a.take("p", false);
        assert_eq!(r.len(), MAX_UNREAD);
        assert_eq!(r[0].body.as_deref(), Some("3"));
    }
}
