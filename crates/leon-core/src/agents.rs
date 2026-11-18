//! Agents started by other agents (docs/agents.md, "Orchestrate other agents"): which block works
//! for which, the reports waiting for each orchestrator, and the state each block's program last
//! reported, which looking at a pane doesn't clear.

use std::collections::{HashMap, VecDeque};
use std::time::Instant;

use serde::{Deserialize, Serialize};

use crate::model::State;
use crate::program_status::{Kind, Reason, Record, State as Status};

/// Unread reports an orchestrator keeps; past it the oldest goes.
pub const MAX_UNREAD: usize = 256;
/// Between typed text and its Enter: an agent's prompt can take text and a return that arrive
/// together as a paste, where the return is a new line instead of submitting.
pub const ENTER_AFTER: std::time::Duration = std::time::Duration::from_millis(300);

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

/// A git worktree and branch made for a worker (`leon agent start --worktree`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Worktree {
    pub path: String,
    pub branch: String,
    /// The checkout it was made from, which removes it.
    pub repo: String,
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

/// How a worker left its team.
enum Ended {
    /// Its orchestrator's block went: nobody to tell.
    Orphaned,
    /// Its program exited; its block stays open.
    Exited(i32),
    /// Its block closed, with its program's exit status when known.
    Closed(Option<i32>),
}

#[derive(Debug, Default)]
pub struct Inbox {
    pub unread: VecDeque<Report>,
    /// A report came since the last notice was typed or the inbox read.
    pub owed: bool,
}

/// What a block's program last reported for its root record.
#[derive(Debug, Clone)]
pub struct Last {
    pub state: Status,
    pub kind: Option<Kind>,
    pub at: Instant,
    /// The terminal's foreground process group then, when known: the report holds only while it is
    /// still in front. A group, not a name: a program that execs keeps its group, and one that
    /// quits back to its shell gives way to the shell's.
    front: Option<i32>,
}

#[derive(Debug, Default)]
pub struct Agents {
    /// By worker block.
    pub workers: HashMap<String, Worker>,
    /// By orchestrator block.
    pub inboxes: HashMap<String, Inbox>,
    /// By block: what its program last reported for its root record.
    last: HashMap<String, Last>,
    /// By block: the worktree made for it, kept until the block closes (`leon agent close`
    /// removes the worktree; any other close leaves it).
    pub worktrees: HashMap<String, Worktree>,
}

pub fn now_secs() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

impl Agents {
    /// A report for `parent`: dropped when it says nothing new (the worker's newest unread report
    /// has the same state and kind, and the same body or this one has none), else kept, the
    /// oldest going past the cap.
    pub fn push(&mut self, parent: &str, r: Report) {
        let inbox = self.inboxes.entry(parent.to_string()).or_default();
        let newest = inbox.unread.iter().rev().find(|o| o.block_id == r.block_id);
        let same = |o: &Report| o.state == r.state && o.kind == r.kind && (r.body.is_none() || o.body == r.body);
        if newest.is_some_and(same) {
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
        if record.state == Status::Blocked && record.kind == Some(Kind::Permission) {
            if let Ok((_, b)) = self.block(id) {
                b.terminal.hold_for_person();
            }
        }
        let front = self.block(id).ok().and_then(|(_, b)| b.terminal.foreground_group());
        let last = Last { state: record.state, kind: record.kind, at: Instant::now(), front };
        self.agents.last.insert(id.to_string(), last);
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

    /// What block `id`'s program last reported, while that program is still in front: an agent
    /// that quit back to its shell said nothing about the shell.
    pub fn agent_state(&self, id: &str) -> Option<&Last> {
        let last = self.agents.last.get(id)?;
        let front = self.block(id).ok()?.1.terminal.foreground_group();
        (last.front.is_none() || last.front == front).then_some(last)
    }

    /// Block `id`'s root record went for `reason`. Looking at the pane changes nothing the program
    /// said; its program clearing it says it is idle; an exit, a new shell prompt or a reset mean
    /// what it said no longer holds.
    pub fn agent_cleared(&mut self, id: &str, reason: Reason) {
        match reason {
            Reason::Seen | Reason::Evicted | Reason::Report => {}
            Reason::Clear => {
                if let Some(l) = self.agents.last.get_mut(id) {
                    (l.state, l.kind, l.at) = (Status::Idle, None, Instant::now());
                }
            }
            Reason::Exited | Reason::Prompt | Reason::Reset => {
                self.agents.last.remove(id);
            }
        }
    }

    /// Settle what blocks that went or whose program ended leave behind: a worker that exited or
    /// closed reports so and leaves its team; an orchestrator that closed loses its inbox and its
    /// workers' links; a closed block's last state goes.
    pub fn prune_agents(&mut self) {
        let a = &self.agents;
        if a.workers.is_empty() && a.inboxes.is_empty() && a.last.is_empty() && a.worktrees.is_empty() {
            return;
        }
        // each live block, and its program's exit status if it exited
        let live: HashMap<&str, Option<i32>> =
            self.sessions.iter().flat_map(|s| &s.blocks).map(|b| (b.id.as_str(), b.exit_code)).collect();
        let mut ended = vec![];
        for (id, w) in &self.agents.workers {
            let how = match (live.get(w.parent.as_str()), live.get(id.as_str())) {
                (None, _) => Ended::Orphaned,
                (_, None) => Ended::Closed(self.exit_status(id)),
                (_, Some(Some(code))) => Ended::Exited(*code),
                (_, Some(None)) => continue,
            };
            ended.push((id.clone(), how));
        }
        let keys = self.agents.inboxes.keys().chain(self.agents.last.keys()).chain(self.agents.worktrees.keys());
        let gone: Vec<String> = keys.filter(|id| !live.contains_key(id.as_str())).cloned().collect();
        let mut kept = HashMap::new(); // worktrees of closed blocks, for their reports
        for id in gone {
            self.agents.inboxes.remove(&id);
            self.agents.last.remove(&id);
            if let Some(w) = self.agents.worktrees.remove(&id) {
                kept.insert(id, w);
            }
        }
        for (id, how) in ended {
            let w = self.agents.workers.remove(&id).expect("listed above");
            let (state, code) = match how {
                Ended::Orphaned => continue,
                Ended::Exited(code) => ("exited", Some(code)),
                Ended::Closed(code) => ("closed", code),
            };
            let report = Report {
                block_id: id.clone(),
                label: self.block(&id).ok().and_then(|(_, b)| b.label.clone()).or(w.label),
                agent: w.agent,
                state: state.into(),
                kind: None,
                title: None,
                body: body(code, kept.get(&id)),
                at: now_secs(),
            };
            self.agents.push(&w.parent, report);
        }
    }
}

/// A report's body for a worker that ended: its exit status, and the worktree a close left.
fn body(code: Option<i32>, kept: Option<&Worktree>) -> Option<String> {
    let code = code.map(|c| format!("exit status {c}"));
    // neutral: `leon agent close` removes it just after, any other close leaves it
    let tree = kept.map(|w| format!("its worktree: {} (branch {})", w.path, w.branch));
    let parts: Vec<String> = code.into_iter().chain(tree).collect();
    (!parts.is_empty()).then(|| parts.join("; "))
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
        a.push("p", report("w", "error", Some("x")));
        a.push("p", report("w", "error", Some("x")));
        let bodies: Vec<_> = a.take("p", true).into_iter().map(|r| (r.block_id, r.state, r.body)).collect();
        assert_eq!(
            bodies,
            [
                ("w".into(), "done".into(), Some("result".into())),
                ("other".into(), "done".into(), None),
                ("w".into(), "done".into(), Some("second turn".into())),
                ("w".into(), "error".into(), None),
                ("w".into(), "error".into(), Some("x".into())),
            ]
        );
        assert!(a.inboxes["p"].owed);
        assert_eq!(a.unread_from("p", "w"), 4);
        assert_eq!(a.take("p", false).len(), 5);
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
