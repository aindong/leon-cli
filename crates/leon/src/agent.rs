//! `leon agent start|ls|tell` and `leon inbox`: agents starting agents that report back to them
//! (docs/agents.md, "Orchestrate other agents").

use std::io::Read;
use std::path::Path;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use std::path::PathBuf;

use clap::Subcommand;

use crate::blocks::{block_query, pick_block};
use crate::client::{Client, Fail};
use crate::commands::print_json;
use crate::{Cli, Cmd};

/// A task bigger than this is refused: it goes on the agent's command line.
const MAX_TASK: usize = 64 << 10;

/// What leon adds after the task: how the worker reports.
const PREAMBLE: &str = "You were started by another agent through leon, and you report to it through leon. \
Your progress, your permission prompts and the end of each turn reach it on their own; a person \
answers permission prompts, not that agent. When you finish, report the result in a line or two, \
then end your turn: leon notify --state done --body \"<result>\". If you need an answer from it, \
ask and end your turn: leon notify --state blocked --kind question --body \"<question>\"; its \
answer is typed into your prompt. `leon skill show orchestration` has the details.";

#[derive(Subcommand)]
pub enum AgentCmd {
    /// Start AGENT with TASK as its first prompt, working for you (-b, else $LEON_BLOCK): its
    /// reports go to your `leon inbox`. Prints its block ID.
    Start {
        /// Label for the worker's block.
        #[arg(long)]
        label: Option<String>,
        /// Open it in a pane beside you instead of a new tab.
        #[arg(long = "split", value_enum, value_name = "DIR")]
        split: Option<crate::Edge>,
        /// Working directory [default: the current directory].
        #[arg(short = 'c', long = "cwd")]
        cwd: Option<PathBuf>,
        /// Give it a git worktree and branch of its own, beside the checkout (-c DIR, else here);
        /// `leon agent close` removes them.
        #[arg(long)]
        worktree: bool,
        /// The worktree's new branch [default: agent/LABEL, else agent/AGENT-TIME-PID].
        #[arg(long, value_name = "NAME", requires = "worktree")]
        branch: Option<String>,
        /// Read the task from FILE.
        #[arg(short = 'f', long, value_name = "FILE", conflicts_with = "task")]
        file: Option<PathBuf>,
        /// The agent's program: claude, codex, or any program that takes a prompt as its last argument.
        agent: String,
        /// The task, given to the agent as its first prompt; - reads stdin.
        task: Option<String>,
        /// More arguments for the agent, after `--`; they go before the prompt.
        #[arg(last = true, value_name = "ARGS")]
        args: Vec<String>,
    },
    /// List the agents working for you: label, state, agent, age, unread reports, block.
    Ls,
    /// Close a worker, and remove its worktree and branch; refused while they hold work that
    /// isn't committed and pushed (or on another branch), unless --force.
    Close {
        /// The worker's block.
        #[arg(value_name = "BLOCK")]
        target: String,
        /// Remove the worktree and branch even with work in them, which is then lost.
        #[arg(long)]
        force: bool,
    },
    /// Type TEXT and press Enter in a worker, once its agent reports its turn ended.
    Tell {
        /// The worker's block.
        #[arg(value_name = "BLOCK")]
        target: String,
        /// The text to type.
        text: String,
        /// Give up after this long (e.g. 30s, 10m) and exit 1.
        #[arg(long = "for", default_value = "10m", value_parser = crate::blocks::parse_duration)]
        timeout: std::time::Duration,
    },
}

pub fn run(c: &mut Client, cli: &Cli) -> Result<(), Fail> {
    match cli.cmd() {
        Cmd::Agent(AgentCmd::Start { label, split, cwd, worktree, branch, file, agent, task, args }) => {
            if block_query(cli, None).is_none() {
                return Err(Fail::Usage(
                    "no block for the worker to report to: run inside leon ($LEON_BLOCK) or pass -b BLOCK".into(),
                ));
            }
            if *worktree && cli.host.is_some() {
                return Err(Fail::Usage("--worktree runs git here; it can't make one on another machine (-H)".into()));
            }
            let task = read_task(file.as_deref(), task.as_deref())?;
            let (sid, parent) = pick_block(c, cli, None)?;
            let name = Path::new(agent).file_name().map_or(agent.clone(), |n| n.to_string_lossy().into_owned());
            let mut cwd = crate::commands::block_cwd(cli, cwd.as_deref())?;
            let tree = match worktree {
                false => None,
                true => {
                    let here =
                        cwd.clone().map_or_else(std::env::current_dir, Ok).map_err(|e| Fail::Other(e.to_string()))?;
                    let default = || match label {
                        Some(l) => format!("agent/{l}"),
                        None => format!("agent/{name}-{}", leon_core::agents::now_secs()),
                    };
                    let branch = crate::agent_worktree::branch_name(&branch.clone().unwrap_or_else(default));
                    let tree = crate::agent_worktree::create(&here, &branch)?;
                    cwd = tree["path"].as_str().map(PathBuf::from);
                    Some(tree)
                }
            };
            let mut command: Vec<String> = std::iter::once(agent.clone()).chain(args.iter().cloned()).collect();
            // a prompt starting with `-` would be read as an option
            let lead = if task.starts_with('-') { "Task:\n" } else { "" };
            command.push(format!("{lead}{}\n\n{PREAMBLE}", task.trim_end()));
            // kept open after it exits, so its last screen can still be read; the orchestrator closes it
            let options = json!({ "command": command, "cwd": cwd, "exit": { "on_completion": false } });
            let layout = json!({ "block": { "label": label, "options": options } });
            let started = match split {
                None => c.call("session.new_window", json!({ "session_id": sid, "layout": layout })),
                Some(edge) => {
                    let (direction, side) = crate::layout::place(*edge);
                    let params = json!({ "session_id": sid, "anchor_id": parent, "direction": direction,
                        "side": side, "ratio": crate::layout::ratio(side, 50), "focus": false, "layout": layout });
                    c.call("session.new_split", params)
                }
            };
            let r = match (started, &tree) {
                (Ok(r), _) => r,
                (Err(e), Some(t)) => {
                    // just made, so it holds nothing
                    if let Err(left) = crate::agent_worktree::remove(t, false) {
                        let hand = crate::agent_worktree::by_hand(t);
                        eprintln!(
                            "leon: the worktree made for it stays ({}); to remove it: {hand}",
                            crate::commands::message(&left)
                        );
                    }
                    return Err(e);
                }
                (Err(e), None) => return Err(e),
            };
            let id = r["block_id"].as_str().unwrap_or_default().to_string();
            c.call("agent.link", json!({ "block_id": id, "parent_id": parent, "agent": name, "worktree": tree }))?;
            println!("{id}");
        }
        Cmd::Agent(AgentCmd::Ls) => {
            let (_, me) = pick_block(c, cli, None)?;
            let team = c.call("agent.list", json!({ "block_id": me }))?;
            if cli.json {
                print_json(&team);
                return Ok(());
            }
            let now = leon_core::agents::now_secs();
            for w in team["workers"].as_array().into_iter().flatten() {
                let age = now.saturating_sub(w["started_at"].as_u64().unwrap_or(now));
                println!(
                    "{:<16} {:<18} {:<10} {:>6}  {} unread  {}",
                    w["label"].as_str().unwrap_or("-"),
                    state(w),
                    w["agent"].as_str().unwrap_or(""),
                    ago(age),
                    w["unread"],
                    w["block_id"].as_str().unwrap_or("")
                );
            }
        }
        Cmd::Agent(AgentCmd::Close { target, force }) => {
            let (sid, w) = pick_block(c, cli, Some(target))?;
            let tree = c.call("agent.list", json!({ "block_id": w }))?["worktree"].clone();
            let close = |c: &mut Client| c.call("block.close", json!({ "session_id": sid, "block_id": w }));
            if tree.is_null() {
                close(c)?;
                return Ok(());
            }
            if cli.host.is_some() {
                return Err(Fail::Usage(format!(
                    "{w} has a worktree on the machine its server runs on: run `leon agent close` there; nothing was closed"
                )));
            }
            let path = tree["path"].as_str().unwrap_or("").to_string();
            crate::agent_worktree::check_ours(&tree)?;
            let refuse = |lost: Vec<String>, closed: bool| {
                let then = match closed {
                    false => "Nothing was closed.".to_string(),
                    true => format!(
                        "The worker is closed; its worktree stays. To remove it: {}",
                        crate::agent_worktree::by_hand(&tree)
                    ),
                };
                Fail::Other(format!(
                    "{w}'s worktree {path} holds work that removing it would lose:\n  {}\nCommit and push it (or \
                     merge it into another branch), or pass --force to remove it anyway. {then}",
                    lost.join("\n  ")
                ))
            };
            if !force {
                let lost = crate::agent_worktree::would_lose(&tree)?;
                if !lost.is_empty() {
                    return Err(refuse(lost, false));
                }
            }
            close(c)?;
            if !force {
                // the agent may have written as it ended
                std::thread::sleep(Duration::from_millis(300));
                let lost = crate::agent_worktree::would_lose(&tree)?;
                if !lost.is_empty() {
                    return Err(refuse(lost, true));
                }
            }
            crate::agent_worktree::remove(&tree, *force).map_err(|e| {
                let hand = crate::agent_worktree::by_hand(&tree);
                Fail::Other(format!(
                    "{w} is closed, but removing its worktree failed: {}\nTo finish: {hand}",
                    crate::commands::message(&e)
                ))
            })?;
        }
        Cmd::Agent(AgentCmd::Tell { target, text, timeout }) => {
            let (sid, w) = pick_block(c, cli, Some(target))?;
            let end = Instant::now() + *timeout;
            loop {
                let me = c.call("agent.list", json!({ "block_id": w }))?;
                if me["state"] == "blocked" && me["kind"] == "permission" {
                    return Err(Fail::Other(format!(
                        "{w} waits on a permission prompt, which only a person answers in the app"
                    )));
                }
                // null: its agent never reported, or quit back to its shell, which mustn't get the text
                if !me["state"].is_null() && me["state"] != "working" {
                    break;
                }
                if Instant::now() >= end {
                    let why = if me["state"].is_null() {
                        "reports no state (no leon hooks, or its agent quit)"
                    } else {
                        "is still working"
                    };
                    return Err(Fail::Other(format!("{w} {why}; nothing was typed")));
                }
                std::thread::sleep(Duration::from_millis(250));
            }
            let method = |m: &str, args: Value| json!({ "session_id": sid, "block_id": w, "method": m, "args": args });
            c.call("block.method", method("dev.leon.terminal.write", json!({ "data": text })))?;
            std::thread::sleep(leon_core::agents::ENTER_AFTER);
            c.call(
                "block.event",
                json!({ "session_id": sid, "block_id": w, "event": { "type": "key", "key": "enter" } }),
            )?;
        }
        Cmd::Inbox { peek } => {
            let (_, me) = pick_block(c, cli, None)?;
            let r = c.call("agent.inbox", json!({ "block_id": me, "peek": peek }))?;
            let reports = r["reports"].as_array().cloned().unwrap_or_default();
            if cli.json {
                print_json(&Value::Array(reports));
                return Ok(());
            }
            if reports.is_empty() {
                println!("no new reports");
            }
            for x in &reports {
                let block = x["block_id"].as_str().unwrap_or("");
                let who = x["label"].as_str().unwrap_or(block);
                let what =
                    [x["title"].as_str(), x["body"].as_str()].into_iter().flatten().collect::<Vec<_>>().join(": ");
                println!("{who} ({}, {block}) {}: {what}", x["agent"].as_str().unwrap_or(""), state(x));
                if x["kind"] == "permission" {
                    println!("  only the user answers this, in the app; you can't type into it until then");
                }
                println!("  look: leon capture -b {block} --trim | tail -40");
            }
        }
        _ => unreachable!("commands::run routes only agent commands here"),
    }
    Ok(())
}

/// `state` or `state/kind` of a worker or a report.
fn state(v: &Value) -> String {
    let s = v["state"].as_str().unwrap_or("-");
    v["kind"].as_str().map_or_else(|| s.to_string(), |k| format!("{s}/{k}"))
}

fn ago(secs: u64) -> String {
    match secs {
        0..=59 => format!("{secs}s"),
        60..=3599 => format!("{}m", secs / 60),
        _ => format!("{}h", secs / 3600),
    }
}

/// At most one byte past `MAX_TASK` of `r`, so an oversized task is refused without reading it all.
fn read_capped(r: impl Read) -> std::io::Result<String> {
    let mut s = String::new();
    r.take(MAX_TASK as u64 + 1).read_to_string(&mut s)?;
    Ok(s)
}

/// The task: `--file`, `-` for stdin, or the text.
fn read_task(file: Option<&Path>, task: Option<&str>) -> Result<String, Fail> {
    let text = match (file, task) {
        (Some(f), _) => {
            let file = std::fs::File::open(f).map_err(|e| Fail::Other(format!("{}: {e}", f.display())))?;
            read_capped(file).map_err(|e| Fail::Other(format!("{}: {e}", f.display())))?
        }
        (None, Some("-")) => read_capped(std::io::stdin()).map_err(|e| Fail::Other(e.to_string()))?,
        (None, Some(t)) => t.to_string(),
        (None, None) => return Err(Fail::Usage("give the task: TEXT, --file FILE, or - for stdin".into())),
    };
    if text.trim().is_empty() {
        return Err(Fail::Usage("the task is empty".into()));
    }
    if text.len() > MAX_TASK {
        return Err(Fail::Usage(format!(
            "the task is over {} KiB: put it in a file and name the file in it",
            MAX_TASK >> 10
        )));
    }
    Ok(text)
}
