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
/// How long `agent tell` waits for the worker's turn to end, unless `--for`.
const TELL_WAIT: Duration = Duration::from_secs(600);
/// Between the text and its Enter: an agent's prompt takes a text and a return in one read as a paste.
const ENTER_AFTER: Duration = Duration::from_millis(300);

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
    /// Type TEXT and press Enter in a worker, once its turn ended (it no longer reports working).
    Tell {
        /// The worker's block.
        #[arg(value_name = "BLOCK")]
        target: String,
        /// The text to type.
        text: String,
        /// Give up after this long (e.g. 30s, 10m) and exit 1 [default: 10m].
        #[arg(long = "for", value_parser = crate::blocks::parse_duration)]
        timeout: Option<std::time::Duration>,
    },
}

pub fn run(c: &mut Client, cli: &Cli) -> Result<(), Fail> {
    match cli.cmd() {
        Cmd::Agent(AgentCmd::Start { label, split, cwd, file, agent, task, args }) => {
            if block_query(cli, None).is_none() {
                return Err(Fail::Usage(
                    "no block for the worker to report to: run inside leon ($LEON_BLOCK) or pass -b BLOCK".into(),
                ));
            }
            let task = read_task(file.as_deref(), task.as_deref())?;
            let (sid, parent) = pick_block(c, cli, None)?;
            let cwd = crate::commands::block_cwd(cli, cwd.as_deref())?;
            let mut command: Vec<String> = std::iter::once(agent.clone()).chain(args.iter().cloned()).collect();
            command.push(format!("{}\n\n{PREAMBLE}", task.trim_end()));
            // kept open after it exits, so its last screen can still be read; the orchestrator closes it
            let options = json!({ "command": command, "cwd": cwd, "exit": { "on_completion": false } });
            let layout = json!({ "block": { "label": label, "options": options } });
            let r = match split {
                None => c.call("session.new_window", json!({ "session_id": sid, "layout": layout }))?,
                Some(edge) => {
                    let (direction, side) = crate::layout::place(*edge);
                    let params = json!({ "session_id": sid, "anchor_id": parent, "direction": direction,
                        "side": side, "ratio": crate::layout::ratio(side, 50), "focus": false, "layout": layout });
                    c.call("session.new_split", params)?
                }
            };
            let id = r["block_id"].as_str().unwrap_or_default().to_string();
            let name = Path::new(agent).file_name().map_or(agent.clone(), |n| n.to_string_lossy().into_owned());
            c.call("agent.link", json!({ "block_id": id, "parent_id": parent, "agent": name }))?;
            println!("{id}");
        }
        Cmd::Agent(AgentCmd::Ls) => {
            let (_, me) = pick_block(c, cli, None)?;
            let team = c.call("agent.list", json!({ "block_id": me }))?;
            if cli.json {
                print_json(&team);
                return Ok(());
            }
            let now = now();
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
        Cmd::Agent(AgentCmd::Tell { target, text, timeout }) => {
            let (sid, w) = pick_block(c, cli, Some(target))?;
            let end = Instant::now() + timeout.unwrap_or(TELL_WAIT);
            loop {
                let me = c.call("agent.list", json!({ "block_id": w }))?;
                if me["state"] == "blocked" && me["kind"] == "permission" {
                    return Err(Fail::Other(format!(
                        "{w} waits on a permission prompt, which only a person answers in the app"
                    )));
                }
                if me["state"] != "working" {
                    break;
                }
                if Instant::now() >= end {
                    return Err(Fail::Other(format!("{w} is still working; nothing was typed")));
                }
                std::thread::sleep(Duration::from_millis(250));
            }
            let method = |m: &str, args: Value| json!({ "session_id": sid, "block_id": w, "method": m, "args": args });
            c.call("block.method", method("dev.leon.terminal.write", json!({ "data": text })))?;
            std::thread::sleep(ENTER_AFTER);
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

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// The task: `--file`, `-` for stdin, or the text.
fn read_task(file: Option<&Path>, task: Option<&str>) -> Result<String, Fail> {
    let text = match (file, task) {
        (Some(f), _) => std::fs::read_to_string(f).map_err(|e| Fail::Other(format!("{}: {e}", f.display())))?,
        (None, Some("-")) => {
            let mut s = String::new();
            std::io::stdin()
                .take(MAX_TASK as u64 + 1)
                .read_to_string(&mut s)
                .map_err(|e| Fail::Other(e.to_string()))?;
            s
        }
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
