//! Agents orchestrating agents (docs/agents.md): `agent.link`, `agent.list` and `agent.inbox`, the
//! notice leon types into an idle orchestrator, and the permission guard the socket loop applies.

use std::sync::Arc;
use std::time::{Duration, Instant};

use leon_core::agents::{now_secs, Report, Worker};
use leon_core::model::clean_label;
use leon_core::program_status::{Kind, State as Status};
use leon_core::{ApiError, State};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::methods::Empty;
use crate::{Ctx, Registry};

/// An orchestrator's turn must have ended this long ago before a notice is typed: its prompt is back.
const SETTLED: Duration = Duration::from_secs(1);
/// No input for this long before a notice is typed, so it never joins what a person is typing.
const QUIET: Duration = Duration::from_secs(3);
/// Between the notice's text and its Enter: a text and a return in one read look like a paste.
const ENTER_AFTER: Duration = Duration::from_millis(300);

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Link {
    /// The worker's block.
    pub block_id: String,
    /// The block it reports to.
    pub parent_id: String,
    /// The agent's program name, e.g. `claude`.
    pub agent: String,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Of {
    pub block_id: String,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReadInbox {
    pub block_id: String,
    /// List the reports without marking them read.
    #[serde(default)]
    pub peek: bool,
}

#[derive(Serialize, JsonSchema)]
pub struct WorkerView {
    pub block_id: String,
    pub label: Option<String>,
    pub agent: String,
    /// Seconds since the epoch.
    pub started_at: u64,
    /// The state its program last reported (`working`, `blocked`, `done`, `error`, `idle`); null
    /// before its first report.
    pub state: Option<Status>,
    pub kind: Option<Kind>,
    /// Its reports the orchestrator hasn't read.
    pub unread: usize,
}

#[derive(Serialize, JsonSchema)]
pub struct Team {
    /// The block this one works for, if any.
    pub parent: Option<String>,
    /// The state this block's own program last reported, and its kind.
    pub state: Option<Status>,
    pub kind: Option<Kind>,
    /// The blocks that work for this one, oldest first.
    pub workers: Vec<WorkerView>,
}

#[derive(Serialize, JsonSchema)]
pub struct Reports {
    /// Oldest first. A report's `state` is `done`, `blocked` or `error` as the worker reported it,
    /// or `exited`/`closed` when it ended; `body` is its message.
    pub reports: Vec<Report>,
}

pub fn add(r: &mut Registry) {
    r.add(
        "agent.link",
        "Make `block_id` a worker of `parent_id`: its status reports (done, blocked, error) and its end \
         go to the parent's inbox. `leon agent start` calls it.",
        |ctx, p: Link| {
            let mut st = ctx.state();
            st.prune_agents();
            let label = st.block(&p.block_id)?.1.label.clone();
            st.block(&p.parent_id)?;
            let agent = clean_label(Some(&p.agent))?.ok_or_else(|| ApiError::bad_request("agent is empty"))?;
            let mut up = Some(p.parent_id.clone());
            while let Some(b) = up {
                if b == p.block_id {
                    return Err(ApiError::bad_request("a block can't work for itself or its own workers"));
                }
                up = st.agents.workers.get(&b).map(|w| w.parent.clone());
            }
            let worker = Worker { parent: p.parent_id, agent, label, started_at: now_secs() };
            st.agents.workers.insert(p.block_id.clone(), worker);
            // what it reported before the link counts too
            let root = st.block(&p.block_id)?.1.terminal.program_status().into_iter().find(|(id, _)| id.is_none());
            if let Some((_, record)) = root {
                st.agent_reported(&p.block_id, &record);
            }
            Ok(Empty {})
        },
    );
    r.add(
        "agent.list",
        "A block's parent, the state its program last reported, and its workers with theirs and their \
         unread reports.",
        |ctx, p: Of| {
            let mut st = ctx.state();
            st.prune_agents();
            st.block(&p.block_id)?;
            Ok(team(&st, &p.block_id))
        },
    );
    r.add(
        "agent.inbox",
        "The reports a block's workers sent it, oldest first; they are marked read unless `peek`.",
        |ctx, p: ReadInbox| {
            let mut st = ctx.state();
            st.prune_agents();
            st.block(&p.block_id)?;
            Ok(Reports { reports: st.agents.take(&p.block_id, p.peek) })
        },
    );
}

fn team(st: &State, id: &str) -> Team {
    let last = st.agents.last(id);
    let mut workers: Vec<WorkerView> = st
        .agents
        .workers
        .iter()
        .filter(|(_, w)| w.parent == id)
        .map(|(b, w)| {
            let last = st.agents.last(b);
            WorkerView {
                block_id: b.clone(),
                label: st.block(b).ok().and_then(|(_, x)| x.label.clone()),
                agent: w.agent.clone(),
                started_at: w.started_at,
                state: last.map(|l| l.0),
                kind: last.and_then(|l| l.1),
                unread: st.agents.unread_from(id, b),
            }
        })
        .collect();
    // in the order they were started: blocks' creation order
    let order = |b: &str| st.sessions.iter().flat_map(|s| &s.blocks).position(|x| x.id == b);
    workers.sort_by_key(|w| order(&w.block_id));
    Team {
        parent: st.agents.workers.get(id).map(|w| w.parent.clone()),
        state: last.map(|l| l.0),
        kind: last.and_then(|l| l.1),
        workers,
    }
}

/// Type "You have N leon messages. Run `leon inbox`." into each orchestrator that is owed one and
/// is idle at its prompt: its last report says its turn ended (`done` or `idle`) a moment ago, and
/// nobody typed into it for `QUIET`. A pane that never reported isn't an agent's: never typed into.
pub fn deliver(ctx: &Arc<Ctx>) {
    let due = {
        let mut st = ctx.state();
        if st.agents.inboxes.is_empty() && st.agents.workers.is_empty() {
            return;
        }
        st.prune_agents();
        let now = Instant::now();
        let mut due = vec![];
        let owed: Vec<String> =
            st.agents.inboxes.iter().filter(|(_, i)| i.owed && !i.unread.is_empty()).map(|(b, _)| b.clone()).collect();
        for id in owed {
            let idle = st
                .agents
                .last(&id)
                .is_some_and(|(s, _, at)| matches!(s, Status::Done | Status::Idle) && now >= at + SETTLED);
            let Ok((_, b)) = st.block(&id) else { continue };
            let quiet = b.terminal.last_input().is_none_or(|t| now >= t + QUIET);
            if idle && quiet && b.exit_code.is_none() {
                let t = b.terminal.clone();
                let inbox = st.agents.inboxes.get_mut(&id).expect("listed above");
                inbox.owed = false;
                due.push((t, inbox.unread.len()));
            }
        }
        due
    };
    for (t, n) in due {
        let s = if n == 1 { "" } else { "s" };
        if t.write(format!("You have {n} leon message{s}. Run `leon inbox`.").as_bytes()).is_err() {
            continue;
        }
        let typed = t.last_input();
        tokio::spawn(async move {
            tokio::time::sleep(ENTER_AFTER).await;
            // someone typed meanwhile: leave the line to them
            if t.last_input() == typed {
                let _ = t.key("enter");
            }
        });
    }
}

/// The block an input request types into: `block.event` but `focus`, `block.method` `write`, and
/// `client.key` (to its block, else the session's focused one).
fn input_target(ctx: &Ctx, method: &str, params: &Value) -> Option<String> {
    let block = params["block_id"].as_str().map(String::from);
    match method {
        "block.event" if params["event"]["type"] != "focus" => block,
        "block.method" if params["method"] == "dev.leon.terminal.write" => block,
        "client.key" => block.or_else(|| {
            let st = ctx.state();
            let s = st.session(params["session_id"].as_str()?).ok()?;
            s.windows.get(s.active).and_then(|w| w.focus.clone())
        }),
        _ => None,
    }
}

/// Refuse a request that would answer a permission prompt for an agent: input to a block waiting
/// on one, or a status report that would lift that record, from a program running inside a leon
/// block, which is where agents run. Its own program and what it runs (its hooks) may lift it. The
/// app, `leon attach` in a terminal, a person's own shell, and other machines (`peer` None, or a
/// host's `leon server proxy`) run outside leon blocks: a person is there.
pub fn guard(ctx: &Ctx, peer: Option<i32>, method: &str, params: &Value) -> Result<(), ApiError> {
    let lifts = method == "block.method"
        && params["method"] == "dev.leon.terminal.report_status"
        && !(params["args"]["state"] == "blocked" && params["args"]["kind"] == "permission");
    let target = if lifts { params["block_id"].as_str().map(String::from) } else { input_target(ctx, method, params) };
    let (Some(id), Some(peer)) = (target, peer) else { return Ok(()) };
    let t = match ctx.state().block(&id) {
        Ok((_, b)) => b.terminal.clone(),
        Err(_) => return Ok(()), // the method says not found
    };
    if !t.awaits_permission() {
        return Ok(());
    }
    use leon_core::process_tree::descends_from;
    let programs: Vec<i32> = ctx.state().sessions.iter().flat_map(|s| &s.blocks).map(|b| b.terminal.pid()).collect();
    if !descends_from(peer, &programs) || (lifts && descends_from(peer, &[t.pid()])) {
        return Ok(());
    }
    let what = if lifts { "change its status" } else { "type into it" };
    Err(ApiError::new(
        403,
        "permission_pending",
        format!(
            "{id} waits on a permission prompt, which only a person answers (in the leon app, or \
             `leon attach` in a terminal outside leon); a program inside a leon terminal can't {what} now"
        ),
    ))
}
