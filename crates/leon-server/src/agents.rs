//! Agents orchestrating agents (docs/agents.md): `agent.link`, `agent.list` and `agent.inbox`, the
//! notice leon types into an idle orchestrator, and the permission guard the socket loop applies.

use std::sync::Arc;
use std::time::{Duration, Instant};

use leon_core::agents::{now_secs, Report, Worker, Worktree};
use leon_core::model::clean_label;
use leon_core::program_status::{Kind, State as Status};
use leon_core::{ApiError, State};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::methods::Empty;
use crate::{Ctx, Registry};

/// An orchestrator's turn must have ended at least this long ago before a notice is typed: its
/// prompt is drawn again.
const SETTLED: Duration = Duration::from_secs(1);
/// No input for this long before a notice is typed: a person typing now isn't joined mid-word.
const QUIET: Duration = Duration::from_secs(3);

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Link {
    /// The worker's block.
    pub block_id: String,
    /// The block it reports to.
    pub parent_id: String,
    /// The agent's program name, e.g. `claude`.
    pub agent: String,
    /// The git worktree made for it, kept with the block until it closes.
    pub worktree: Option<Worktree>,
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
    pub worktree: Option<Worktree>,
}

#[derive(Serialize, JsonSchema)]
pub struct Team {
    /// The block this one works for, if any.
    pub parent: Option<String>,
    /// The state this block's own program last reported, and its kind.
    pub state: Option<Status>,
    pub kind: Option<Kind>,
    /// The git worktree made for this block, if any.
    pub worktree: Option<Worktree>,
    /// The blocks that work for this one, oldest first.
    pub workers: Vec<WorkerView>,
}

#[derive(Serialize, JsonSchema)]
pub struct Reports {
    /// Oldest first.
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
            // bounded: a state file edited by hand could hold a loop
            for _ in 0..leon_core::MAX_BLOCKS {
                let Some(b) = up else { break };
                if b == p.block_id {
                    return Err(ApiError::bad_request("a block can't work for itself or its own workers"));
                }
                up = st.agents.workers.get(&b).map(|w| w.parent.clone());
            }
            let worker = Worker { parent: p.parent_id, agent, label, started_at: now_secs() };
            st.agents.workers.insert(p.block_id.clone(), worker);
            if let Some(tree) = p.worktree {
                st.agents.worktrees.insert(p.block_id.clone(), tree);
            }
            // a report it made before the link counts too
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
    let last = st.agent_state(id);
    let mut workers: Vec<WorkerView> = st
        .agents
        .workers
        .iter()
        .filter(|(_, w)| w.parent == id)
        .map(|(b, w)| {
            let last = st.agent_state(b);
            WorkerView {
                block_id: b.clone(),
                label: st.block(b).ok().and_then(|(_, x)| x.label.clone()),
                agent: w.agent.clone(),
                started_at: w.started_at,
                state: last.map(|l| l.state),
                kind: last.and_then(|l| l.kind),
                unread: st.agents.unread_from(id, b),
                worktree: st.agents.worktrees.get(b).cloned(),
            }
        })
        .collect();
    // in the order they were started: blocks' creation order
    let order = |b: &str| st.sessions.iter().flat_map(|s| &s.blocks).position(|x| x.id == b);
    workers.sort_by_key(|w| order(&w.block_id));
    Team {
        parent: st.agents.workers.get(id).map(|w| w.parent.clone()),
        state: last.map(|l| l.state),
        kind: last.and_then(|l| l.kind),
        worktree: st.agents.worktrees.get(id).cloned(),
        workers,
    }
}

/// The line typed into an orchestrator. No backticks or other shell syntax: typed into a shell by
/// mistake, it runs nothing.
pub fn notice(n: usize) -> String {
    match n {
        1 => "You have 1 leon message. Run leon inbox to read it.".into(),
        n => format!("You have {n} leon messages. Run leon inbox to read them."),
    }
}

/// Type `notice` into each owed orchestrator whose program, still in front, last reported its turn
/// ended (`done` or `idle`) at least `SETTLED` ago, and that had no input for `QUIET`. A pane
/// whose program never reported isn't an agent's: it is never typed into.
pub fn deliver(ctx: &Arc<Ctx>) {
    let due = {
        let mut st = ctx.state();
        st.prune_agents();
        if st.agents.inboxes.is_empty() {
            return;
        }
        let now = Instant::now();
        let mut due = vec![];
        let owed: Vec<String> =
            st.agents.inboxes.iter().filter(|(_, i)| i.owed && !i.unread.is_empty()).map(|(b, _)| b.clone()).collect();
        for id in owed {
            let idle = st
                .agent_state(&id)
                .is_some_and(|l| matches!(l.state, Status::Done | Status::Idle) && now >= l.at + SETTLED);
            let Ok((_, b)) = st.block(&id) else { continue };
            let quiet = b.terminal.last_input().is_none_or(|t| now >= t + QUIET);
            // never into a prompt only a person answers, whatever its status says now
            if idle && quiet && b.exit_code.is_none() && !b.terminal.held_for_person() {
                let t = b.terminal.clone();
                let inbox = st.agents.inboxes.get_mut(&id).expect("listed above");
                inbox.owed = false;
                due.push((t, inbox.unread.len()));
            }
        }
        due
    };
    for (t, n) in due {
        let Ok(typed) = t.write_at(notice(n).as_bytes()) else { continue };
        let typed = Some(typed);
        tokio::spawn(async move {
            tokio::time::sleep(leon_core::agents::ENTER_AFTER).await;
            // someone typed meanwhile: leave the line to them
            if t.last_input() == typed && !t.held_for_person() {
                let _ = t.key("enter");
            }
        });
    }
}

/// Who sent a request, for the permission guard.
#[derive(Clone, Copy, Debug)]
pub enum Peer {
    /// A local process, by pid.
    Process(i32),
    /// A local connection whose process the socket didn't name.
    Unknown,
    /// Another machine (Tailscale).
    Remote,
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

/// Whether an input request is typing (keys, a paste, text), as answering a prompt takes, rather
/// than the mouse.
fn typing(method: &str, params: &Value) -> bool {
    method != "block.event" || matches!(params["event"]["type"].as_str(), Some("key" | "paste"))
}

/// Keep agents from answering permission prompts. A block whose program reported one is held
/// for a person (`Terminal::held_for_person`) until a person's keys reach it; meanwhile a program
/// inside a leon terminal, which is where agents run, can't type into it, change its status
/// (only its own program and what that runs, its hooks, may), or run config actions or reloads
/// (Lua in the server types where nothing here sees). A person is a client outside every leon
/// terminal (the app, `leon attach` or a shell in a terminal app) or on another machine; a local
/// client whose process can't be told is taken to be inside.
pub fn guard(ctx: &Ctx, peer: Peer, method: &str, params: &Value) -> Result<(), ApiError> {
    let lua = matches!(method, "client.action" | "server.config_reload");
    let status = method == "block.method" && params["method"] == "dev.leon.terminal.report_status";
    let target = if status { params["block_id"].as_str().map(String::from) } else { input_target(ctx, method, params) };
    let (held, programs) = {
        let st = ctx.state();
        let blocks = || st.sessions.iter().flat_map(|s| &s.blocks);
        let held = match (&target, lua) {
            (Some(id), _) => st.block(id).ok().map(|(_, b)| b.terminal.clone()).filter(|t| t.held_for_person()),
            (None, true) => blocks().map(|b| b.terminal.clone()).find(|t| t.held_for_person()),
            (None, false) => None,
        };
        let Some(held) = held else { return Ok(()) };
        (held, blocks().map(|b| b.terminal.pid()).collect::<Vec<_>>())
    };
    use leon_core::process_tree::inside;
    let person = match peer {
        Peer::Remote => true,
        Peer::Unknown => false,
        Peer::Process(p) => inside(p, &programs) == Some(false),
    };
    if person {
        if target.is_some() && !status && typing(method, params) {
            held.person_typed();
        }
        return Ok(());
    }
    // its own hooks report its state; the hold stays until a person types
    if status && matches!(peer, Peer::Process(p) if inside(p, &[held.pid()]) == Some(true)) {
        return Ok(());
    }
    let id = target.unwrap_or_else(|| "a block".into());
    Err(ApiError::new(
        403,
        "permission_pending",
        format!(
            "{id} waits for a person to answer a permission prompt (in the leon app, or `leon attach` \
             in a terminal outside leon); until one types there, a program inside a leon terminal \
             can't type into it, change its status, or run config actions"
        ),
    ))
}
