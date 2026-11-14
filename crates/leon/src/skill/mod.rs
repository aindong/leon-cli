//! `leon skill`: the guide for coding agents, and installing it into Claude Code and Codex. The
//! installed SKILL.md only says "run `leon skill show`", so the guide always matches the binary.

mod hooks;

use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use crate::client::Fail;
use crate::commands::print_json;
use crate::{Cli, SkillCmd};

const GUIDE: &str = include_str!("guide.md");
const MARKER: &str = "<!-- leon-skill: installed by leon skill setup -->";
const STUB: &str = r#"---
name: leon
description: Drive the leon terminal multiplexer. Open terminals beside you, run commands and wait for them, read other terminals' screens and type into them. Use when LEON_SESSION is set (you run inside leon) or the user mentions leon sessions, tabs, panes or blocks.
---
<!-- leon-skill: installed by leon skill setup -->

Before you use leon, run `leon skill show` and follow the guide it prints. The guide comes from
the installed leon binary, so it always matches the version you are driving.
"#;
const BACKUP: &str = "leon-backup";

#[derive(Clone, Copy, PartialEq, Eq, Debug, clap::ValueEnum)]
pub enum Agent {
    Claude,
    Codex,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum State {
    Current,
    Outdated,
    Missing,
    Foreign,
    Unreadable,
}

impl State {
    fn as_str(self) -> &'static str {
        match self {
            State::Current => "current",
            State::Outdated => "outdated",
            State::Missing => "missing",
            State::Foreign => "foreign",
            State::Unreadable => "unreadable",
        }
    }
}

/// Where one agent's skill and hook live. `hook` is `None` when the agent's home dir doesn't exist
/// and setup wasn't asked for that agent by name: then there is no agent to hook into.
struct Target {
    agent: Agent,
    skill: PathBuf,
    hook: Option<PathBuf>,
}

impl Target {
    fn name(&self) -> &'static str {
        match self.agent {
            Agent::Claude => "claude",
            Agent::Codex => "codex",
        }
    }
}

fn env_dir(name: &str) -> Option<PathBuf> {
    std::env::var_os(name).filter(|v| !v.is_empty()).map(PathBuf::from)
}

/// The agent's home dir and its target. Paths follow spec §6: `$CLAUDE_CONFIG_DIR` (~/.claude) for
/// Claude Code; `$CODEX_HOME` (~/.codex) and ~/.agents/skills for Codex.
fn target(agent: Agent, named: bool) -> Result<(PathBuf, Target), Fail> {
    let home = || env_dir("HOME").ok_or_else(|| Fail::Other("HOME is not set".into()));
    let (dir, skill, hook) = match agent {
        Agent::Claude => {
            let dir = env_dir("CLAUDE_CONFIG_DIR").map_or_else(|| home().map(|h| h.join(".claude")), Ok)?;
            (dir.clone(), dir.join("skills/leon/SKILL.md"), dir.join("settings.json"))
        }
        Agent::Codex => {
            let dir = env_dir("CODEX_HOME").map_or_else(|| home().map(|h| h.join(".codex")), Ok)?;
            (dir.clone(), home()?.join(".agents/skills/leon/SKILL.md"), dir.join("hooks.json"))
        }
    };
    let hook = (named || dir.is_dir()).then_some(hook);
    Ok((dir, Target { agent, skill, hook }))
}

/// The agents a command covers. Setup without `--agent` picks the installed ones; with none found,
/// the Codex skill path alone (~/.agents/skills, read by other agents too). Status and remove cover all.
fn targets(agent: Option<Agent>, for_setup: bool) -> Result<Vec<Target>, Fail> {
    if let Some(a) = agent {
        return Ok(vec![target(a, for_setup)?.1]);
    }
    let all = [Agent::Claude, Agent::Codex].map(|a| target(a, false)).into_iter().collect::<Result<Vec<_>, _>>()?;
    if !for_setup {
        return Ok(all.into_iter().map(|(_, t)| t).collect());
    }
    let found: Vec<Target> = all.into_iter().filter(|(dir, _)| dir.is_dir()).map(|(_, t)| t).collect();
    if found.is_empty() {
        return Ok(vec![target(Agent::Codex, false)?.1]);
    }
    Ok(found)
}

fn skill_state(path: &Path) -> State {
    match std::fs::read_to_string(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => State::Missing,
        Err(_) => State::Unreadable,
        Ok(t) if t == STUB => State::Current,
        Ok(t) if t.contains(MARKER) => State::Outdated,
        Ok(_) => State::Foreign,
    }
}

fn hook_state(read: &Result<Option<Value>, String>) -> State {
    match read {
        Err(_) => State::Unreadable,
        Ok(None) => State::Missing,
        Ok(Some(v)) => hooks::state(v),
    }
}

/// One agent's overall state from its parts.
fn overall(parts: &[State]) -> State {
    let any = |s| parts.contains(&s);
    let all = |s| parts.iter().all(|p| *p == s);
    if any(State::Unreadable) {
        State::Unreadable
    } else if any(State::Foreign) {
        State::Foreign
    } else if all(State::Current) {
        State::Current
    } else if all(State::Missing) {
        State::Missing
    } else {
        State::Outdated
    }
}

fn status_of(t: &Target) -> Value {
    let skill = skill_state(&t.skill);
    let hook = t.hook.as_ref().map(|p| (p, hook_state(&hooks::read(p))));
    let parts: Vec<State> = std::iter::once(skill).chain(hook.map(|(_, s)| s)).collect();
    json!({
        "agent": t.name(),
        "status": overall(&parts).as_str(),
        "skill": { "path": t.skill, "state": skill.as_str() },
        "hook": hook.map(|(p, s)| json!({ "path": p, "state": s.as_str() })),
    })
}

/// One file setup or remove writes (or would, with `--dry-run`).
struct Change {
    path: PathBuf,
    action: &'static str,
    op: Op,
}

enum Op {
    None,
    Write { bytes: Vec<u8>, backup: bool },
    Delete,
}

fn json_bytes(v: &Value) -> Vec<u8> {
    let mut b = serde_json::to_vec_pretty(v).expect("json");
    b.push(b'\n');
    b
}

/// What setup does for one agent, or why it won't touch it.
fn setup_changes(t: &Target, force: bool) -> Result<Vec<Change>, String> {
    let skill = skill_state(&t.skill);
    let path = t.skill.clone();
    let stub = || STUB.as_bytes().to_vec();
    let mut changes = vec![match skill {
        State::Current => Change { path, action: "unchanged", op: Op::None },
        State::Missing => Change { path, action: "create", op: Op::Write { bytes: stub(), backup: false } },
        State::Outdated => Change { path, action: "update", op: Op::Write { bytes: stub(), backup: false } },
        State::Foreign if force => Change { path, action: "replace", op: Op::Write { bytes: stub(), backup: true } },
        State::Foreign => {
            return Err(format!(
                "{} is not leon's; pass --force to replace it (it is kept as a backup)",
                path.display()
            ))
        }
        State::Unreadable => return Err(format!("can't read {}", path.display())),
    }];
    if let Some(p) = &t.hook {
        let read = hooks::read(p)?;
        let action = match &read {
            Some(v) if hooks::state(v) == State::Current => "unchanged",
            Some(_) => "update",
            None => "create",
        };
        let op = match action {
            "unchanged" => Op::None,
            _ => Op::Write { backup: read.is_some(), bytes: json_bytes(&hooks::with_ours(read)) },
        };
        changes.push(Change { path: p.clone(), action, op });
    }
    Ok(changes)
}

/// What remove does for one agent: only what setup installed; a foreign skill stays.
fn remove_changes(t: &Target) -> Result<Vec<Change>, String> {
    let path = t.skill.clone();
    let mut changes = vec![match skill_state(&path) {
        State::Current | State::Outdated => Change { path, action: "remove", op: Op::Delete },
        State::Missing => Change { path, action: "unchanged", op: Op::None },
        State::Foreign => Change { path, action: "kept (not leon's)", op: Op::None },
        State::Unreadable => return Err(format!("can't read {}", path.display())),
    }];
    if let Some(p) = &t.hook {
        if let Some(v) = hooks::read(p)? {
            let change = if hooks::state(&v) == State::Missing {
                Change { path: p.clone(), action: "unchanged", op: Op::None }
            } else {
                let rest = hooks::without_ours(v);
                // A file that held only our hook and has no backup was created by setup: delete it.
                if rest == json!({}) && !backup_path(p).exists() {
                    Change { path: p.clone(), action: "remove", op: Op::Delete }
                } else {
                    Change {
                        path: p.clone(),
                        action: "update",
                        op: Op::Write { bytes: json_bytes(&rest), backup: true },
                    }
                }
            };
            changes.push(change);
        }
    }
    Ok(changes)
}

fn backup_path(p: &Path) -> PathBuf {
    let mut name = p.file_name().unwrap_or_default().to_os_string();
    name.push(format!(".{BACKUP}"));
    p.with_file_name(name)
}

fn apply(c: &Change) -> std::io::Result<()> {
    match &c.op {
        Op::None => Ok(()),
        Op::Delete => {
            std::fs::remove_file(&c.path)?;
            if c.path.file_name() == Some("SKILL.md".as_ref()) {
                let _ = std::fs::remove_dir(c.path.parent().expect("skill dir"));
                // only if empty
            }
            Ok(())
        }
        Op::Write { bytes, backup } => {
            // A backup is made once, before leon's first change, so it keeps the user's original.
            let b = backup_path(&c.path);
            if *backup && !b.exists() {
                std::fs::copy(&c.path, &b)?;
            }
            write_atomic(&c.path, bytes)
        }
    }
}

/// Write `bytes` to `path` by renaming a temp file over it. A symlink is followed, so a settings
/// file linked from a dotfiles repo stays linked; the old file's permissions are kept.
fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let real = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let dir = real.parent().expect("a file path");
    std::fs::create_dir_all(dir)?;
    let mut name = std::ffi::OsString::from(".");
    name.push(real.file_name().unwrap_or_default());
    name.push(".leon-tmp");
    let tmp = dir.join(name);
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o644)
        .custom_flags(libc::O_NOFOLLOW)
        .open(&tmp)?;
    if let Ok(meta) = std::fs::metadata(&real) {
        f.set_permissions(meta.permissions())?;
    }
    f.write_all(bytes)?;
    f.sync_all()?;
    std::fs::rename(&tmp, &real)
}

fn hook_text() -> Option<String> {
    let var = |n| std::env::var(n).ok().filter(|v| !v.is_empty());
    let (session, block) = (var("LEON_SESSION"), var("LEON_BLOCK"));
    if session.is_none() && block.is_none() {
        return None;
    }
    let or = |v: Option<String>| v.unwrap_or_else(|| "unknown".into());
    Some(format!(
        "You are running inside leon, in session {} and terminal block {}. leon commands act on \
         this session by default. Before you open, read or type into terminals, run `leon skill show` \
         for the guide.",
        or(session),
        or(block)
    ))
}

pub fn run(cli: &Cli, cmd: &SkillCmd) -> Result<(), Fail> {
    match cmd {
        SkillCmd::Show => {
            print!("{GUIDE}");
            Ok(())
        }
        SkillCmd::Hook => {
            if let Some(t) = hook_text() {
                println!("{t}");
            }
            Ok(())
        }
        SkillCmd::Status { agent } => {
            let all: Vec<Value> = targets(*agent, false)?.iter().map(status_of).collect();
            if cli.json {
                print_json(&Value::Array(all));
            } else {
                for s in &all {
                    let hook = s["hook"]["path"].as_str().map(|p| format!("  {p}")).unwrap_or_default();
                    println!(
                        "{:<7} {:<10} {}{hook}",
                        s["agent"].as_str().unwrap_or(""),
                        s["status"].as_str().unwrap_or(""),
                        s["skill"]["path"].as_str().unwrap_or("")
                    );
                }
            }
            Ok(())
        }
        SkillCmd::Setup { agent, dry_run, force } => {
            change_all(cli, &targets(*agent, true)?, *dry_run, |t| setup_changes(t, *force))
        }
        SkillCmd::Remove { agent, dry_run } => change_all(cli, &targets(*agent, false)?, *dry_run, remove_changes),
    }
}

/// Apply (or with `dry_run` only list) each target's changes, then report. Exit 1 if any failed.
fn change_all(
    cli: &Cli,
    targets: &[Target],
    dry_run: bool,
    changes: impl Fn(&Target) -> Result<Vec<Change>, String>,
) -> Result<(), Fail> {
    let mut failed = false;
    let mut report = vec![];
    for t in targets {
        let mut entry = json!({ "agent": t.name() });
        match changes(t) {
            Err(why) => {
                failed = true;
                eprintln!("leon: {}: {why}", t.name());
                entry["error"] = json!(why);
            }
            Ok(list) => {
                for c in &list {
                    if !dry_run {
                        if let Err(e) = apply(c) {
                            failed = true;
                            eprintln!("leon: {}: {e}", c.path.display());
                        }
                    }
                }
                entry["files"] = list.iter().map(|c| json!({ "path": c.path, "action": c.action })).collect();
                if !dry_run
                    && t.agent == Agent::Codex
                    && list.iter().any(|c| t.hook.as_ref() == Some(&c.path) && matches!(c.op, Op::Write { .. }))
                {
                    entry["note"] = json!("Codex asks you to trust new hooks once: run /hooks in Codex.");
                }
            }
        }
        entry["status"] = status_of(t)["status"].clone();
        report.push(entry);
    }
    if cli.json {
        print_json(&Value::Array(report));
    } else {
        for e in &report {
            println!("{}: {}", e["agent"].as_str().unwrap_or(""), e["status"].as_str().unwrap_or(""));
            for f in e["files"].as_array().into_iter().flatten() {
                let action = f["action"].as_str().unwrap_or("");
                let action =
                    if dry_run && action != "unchanged" { format!("would {action}") } else { action.to_string() };
                println!("  {action:<18} {}", f["path"].as_str().unwrap_or(""));
            }
            if let Some(n) = e["note"].as_str() {
                println!("  {n}");
            }
        }
    }
    if failed {
        return Err(Fail::Exit(1));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_stub_carries_the_marker_and_the_guide_its_name() {
        assert!(STUB.contains(MARKER));
        assert_eq!(skill_name(STUB), Some("leon"));
        assert_eq!(skill_name(GUIDE), Some("leon"));
    }

    fn skill_name(text: &str) -> Option<&str> {
        text.lines().find_map(|l| l.strip_prefix("name: "))
    }

    #[test]
    fn overall_state() {
        use State::*;
        assert_eq!(overall(&[Current, Current]), Current);
        assert_eq!(overall(&[Current]), Current);
        assert_eq!(overall(&[Missing, Missing]), Missing);
        assert_eq!(overall(&[Current, Missing]), Outdated);
        assert_eq!(overall(&[Foreign, Current]), Foreign);
        assert_eq!(overall(&[Foreign, Unreadable]), Unreadable);
    }
}
