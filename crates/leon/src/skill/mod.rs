//! `leon skill`: the guide for coding agents, and installing it into Claude Code and Codex. The
//! installed SKILL.md only says "run `leon skill show`", so the guide always matches the binary.

mod files;
mod hooks;
mod resume;

use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use crate::client::Fail;
use crate::commands::print_json;
use crate::{Cli, SkillCmd};
use files::Backup;

const GUIDE: &str = include_str!("guide.md");
const MARKER: &str = "<!-- leon-skill: installed by leon skill setup -->";
const STUB_BODY: &str =
    "Before you use leon, run `leon skill show` and follow the guide it prints. The guide comes from
the installed leon binary, so it always matches the version you are driving.
";

/// The installed SKILL.md: the guide's front matter (so the agent sees the same name and
/// description), our marker on its own line, and a pointer to `leon skill show`.
fn stub() -> String {
    let end = GUIDE[4..].find("\n---\n").map(|i| i + 4 + 5).expect("guide.md starts with front matter");
    format!("{}{MARKER}\n\n{STUB_BODY}", &GUIDE[..end])
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, clap::ValueEnum)]
pub enum Agent {
    Claude,
    Codex,
}

impl Agent {
    const ALL: [Agent; 2] = [Agent::Claude, Agent::Codex];

    fn name(self) -> &'static str {
        match self {
            Agent::Claude => "claude",
            Agent::Codex => "codex",
        }
    }
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
    /// Why setup without `--agent` picked it, for the report.
    why: Option<String>,
}

fn env_dir(name: &str) -> Option<PathBuf> {
    std::env::var_os(name).filter(|v| !v.is_empty()).map(PathBuf::from)
}

/// The agent's home dir and its target. Paths follow spec §6: `$CLAUDE_CONFIG_DIR` (~/.claude) for
/// Claude Code; `$CODEX_HOME` (~/.codex) and ~/.agents/skills for Codex.
fn target(agent: Agent, named: bool) -> Result<(PathBuf, Target), Fail> {
    let home = || leon_core::paths::home().ok_or_else(|| Fail::Other("no home directory: set HOME".into()));
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
    Ok((dir, Target { agent, skill, hook, why: None }))
}

/// The agents a command covers. Setup without `--agent` picks each agent whose home dir exists or
/// whose program is on PATH; with none found, the Codex skill path alone (~/.agents/skills, read by
/// other agents too). Status and remove cover all.
fn targets(agent: Option<Agent>, for_setup: bool) -> Result<Vec<Target>, Fail> {
    if let Some(a) = agent {
        return Ok(vec![target(a, for_setup)?.1]);
    }
    let all = Agent::ALL.map(|a| target(a, false)).into_iter().collect::<Result<Vec<_>, _>>()?;
    if !for_setup {
        return Ok(all.into_iter().map(|(_, t)| t).collect());
    }
    let mut found = vec![];
    for (dir, t) in all {
        let name = t.agent.name();
        let (mut t, why) = match on_path(name) {
            _ if dir.is_dir() => (t, format!("found {}", dir.display())),
            // installed but never run: its home dir comes with the hook
            Some(p) => (target(t.agent, true)?.1, format!("found {name} on PATH ({})", p.display())),
            None => continue,
        };
        t.why = Some(why);
        found.push(t);
    }
    if found.is_empty() {
        let mut t = target(Agent::Codex, false)?.1;
        t.why = Some(
            "no Claude Code or Codex found (not on PATH, no config dir): only the shared skill, which other \
             agents read too; pass --agent claude or --agent codex to add one"
                .into(),
        );
        return Ok(vec![t]);
    }
    Ok(found)
}

/// `program`'s path in a PATH dir, if one holds it as an executable file.
fn on_path(program: &str) -> Option<PathBuf> {
    use std::os::unix::fs::PermissionsExt;
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .filter(|d| d.is_absolute())
        .map(|d| d.join(program))
        .find(|p| std::fs::metadata(p).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0))
}

fn skill_state(path: &Path) -> State {
    match std::fs::read_to_string(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => State::Missing,
        Err(_) => State::Unreadable,
        Ok(t) if t == stub() => State::Current,
        Ok(t) if t.lines().any(|l| l == MARKER) => State::Outdated,
        Ok(_) => State::Foreign,
    }
}

fn hook_state(read: &Result<Option<Value>, String>, agent: Agent) -> State {
    match read {
        Err(_) => State::Unreadable,
        Ok(None) => State::Missing,
        Ok(Some(v)) => hooks::state(v, agent),
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
    let hook = t.hook.as_ref().map(|p| (p, hook_state(&hooks::read(p), t.agent)));
    let parts: Vec<State> = std::iter::once(skill).chain(hook.map(|(_, s)| s)).collect();
    json!({
        "agent": t.agent.name(),
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
    /// `follow`: write through a symlink to its target (settings files), else replace the link.
    Write {
        bytes: Vec<u8>,
        backup: Option<Backup>,
        follow: bool,
    },
    Delete {
        rm_empty_parent: bool,
    },
}

impl Change {
    fn keep(path: &Path, action: &'static str) -> Change {
        Change { path: path.to_path_buf(), action, op: Op::None }
    }
}

fn json_bytes(v: &Value) -> Vec<u8> {
    let mut b = serde_json::to_vec_pretty(v).expect("json");
    b.push(b'\n');
    b
}

/// What setup does for one agent, or why it won't touch it. A SKILL.md is never written through a
/// link: `--force` over a linked foreign skill must not overwrite the file the link points to.
fn setup_changes(t: &Target, force: bool) -> Result<Vec<Change>, String> {
    let path = &t.skill;
    let write = |action, backup| Change {
        path: path.clone(),
        action,
        op: Op::Write { bytes: stub().into_bytes(), backup, follow: false },
    };
    let mut changes = vec![match skill_state(path) {
        State::Current => Change::keep(path, "unchanged"),
        State::Missing => write("create", None),
        State::Outdated => write("update", None),
        State::Foreign if force => write("replace", Some(Backup::Fresh)),
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
        changes.push(match &read {
            Some(v) if hooks::state(v, t.agent) == State::Current => Change::keep(p, "unchanged"),
            _ => Change {
                path: p.clone(),
                action: if read.is_some() { "update" } else { "create" },
                op: Op::Write {
                    backup: read.is_some().then_some(Backup::Once),
                    bytes: json_bytes(&hooks::with_ours(read, t.agent)),
                    follow: true,
                },
            },
        });
    }
    Ok(changes)
}

/// What remove does for one agent: only what setup installed; a foreign skill stays.
fn remove_changes(t: &Target) -> Result<Vec<Change>, String> {
    let path = &t.skill;
    let mut changes = vec![match skill_state(path) {
        State::Current | State::Outdated => {
            Change { path: path.clone(), action: "remove", op: Op::Delete { rm_empty_parent: true } }
        }
        State::Missing => Change::keep(path, "unchanged"),
        State::Foreign => Change::keep(path, "keep (not leon's)"),
        State::Unreadable => return Err(format!("can't read {}", path.display())),
    }];
    if let Some(p) = &t.hook {
        if let Some(v) = hooks::read(p)? {
            changes.push(if hooks::state(&v, t.agent) == State::Missing {
                Change::keep(p, "unchanged")
            } else {
                let rest = hooks::without_ours(v);
                let is_link = std::fs::symlink_metadata(p).is_ok_and(|m| m.file_type().is_symlink());
                // A file that held only our hook and has no backup was created by setup: delete it.
                if rest == json!({}) && !is_link && std::fs::symlink_metadata(files::backup_path(p)).is_err() {
                    Change { path: p.clone(), action: "remove", op: Op::Delete { rm_empty_parent: false } }
                } else {
                    let op = Op::Write { bytes: json_bytes(&rest), backup: Some(Backup::Once), follow: true };
                    Change { path: p.clone(), action: "update", op }
                }
            });
        }
    }
    Ok(changes)
}

fn apply(c: &Change) -> std::io::Result<()> {
    match &c.op {
        Op::None => Ok(()),
        Op::Delete { rm_empty_parent } => {
            std::fs::remove_file(&c.path)?;
            if *rm_empty_parent {
                let _ = std::fs::remove_dir(c.path.parent().expect("a file path"));
                // fails unless empty
            }
            Ok(())
        }
        Op::Write { bytes, backup, follow } => {
            if let Some(how) = backup {
                files::back_up(&c.path, *how)?;
            }
            files::write_atomic(&c.path, bytes, *follow)
        }
    }
}

/// The hook's one line of context. The IDs are checked, since the agent reads this as trusted.
fn hook_text() -> Option<String> {
    let var = |n| std::env::var(n).ok().filter(|v| !v.is_empty());
    let (session, block) = (var("LEON_SESSION"), var("LEON_BLOCK"));
    if session.is_none() && block.is_none() {
        return None;
    }
    let id = |v: Option<String>| {
        v.filter(|v| v.len() <= 64 && v.chars().all(|c| c.is_ascii_alphanumeric() || c == ':' || c == '-'))
            .unwrap_or_else(|| "unknown".into())
    };
    Some(format!(
        "You are running inside leon, in session {} and terminal block {}. leon commands act on \
         this session by default. Before you open, read or type into terminals, run `leon skill show` \
         for the guide.",
        id(session),
        id(block)
    ))
}

pub fn run(cli: &Cli, cmd: &SkillCmd) -> Result<(), Fail> {
    match cmd {
        SkillCmd::Show => {
            print!("{GUIDE}");
            Ok(())
        }
        SkillCmd::Hook { agent } => {
            if let Some(t) = hook_text() {
                println!("{t}");
            }
            if let Some(agent) = agent {
                resume::record(*agent);
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
            change_all(cli, &targets(*agent, true)?, *dry_run, true, |t| setup_changes(t, *force))
        }
        SkillCmd::Remove { agent, dry_run } => {
            change_all(cli, &targets(*agent, false)?, *dry_run, false, remove_changes)
        }
    }
}

/// Apply (or with `dry_run` only list) each target's changes in order, stopping an agent at its
/// first failed write; then report. Exit 1 if anything failed or was refused.
fn change_all(
    cli: &Cli,
    targets: &[Target],
    dry_run: bool,
    setup: bool,
    changes: impl Fn(&Target) -> Result<Vec<Change>, String>,
) -> Result<(), Fail> {
    let mut failed = false;
    let mut report = vec![];
    for t in targets {
        let mut entry = json!({ "agent": t.agent.name() });
        if let Some(why) = &t.why {
            entry["why"] = json!(why);
        }
        let mut notes = vec![];
        match changes(t) {
            Err(why) => {
                failed = true;
                eprintln!("leon: {}: {why}", t.agent.name());
                entry["error"] = json!(why);
            }
            Ok(list) => {
                let mut broke = false;
                let mut files = vec![];
                for c in &list {
                    let action = if broke {
                        "skipped"
                    } else if dry_run {
                        c.action
                    } else if let Err(e) = apply(c) {
                        (broke, failed) = (true, true);
                        eprintln!("leon: {}: {e}", c.path.display());
                        "failed"
                    } else {
                        c.action
                    };
                    let wrote_hook = !dry_run && !broke && t.hook.as_ref() == Some(&c.path);
                    if wrote_hook && setup && t.agent == Agent::Codex && matches!(c.op, Op::Write { .. }) {
                        notes.push("Codex asks you to trust new hooks once: run /hooks in Codex.".to_string());
                    }
                    if wrote_hook && !setup && !matches!(c.op, Op::None) {
                        let b = files::backup_path(&c.path);
                        if b.exists() {
                            notes.push(format!("{} still holds your settings from before setup", b.display()));
                        }
                    }
                    files.push(json!({ "path": c.path, "action": action }));
                }
                entry["files"] = json!(files);
            }
        }
        entry["status"] = status_of(t)["status"].clone();
        if !notes.is_empty() {
            entry["notes"] = json!(notes);
        }
        report.push(entry);
    }
    if cli.json {
        print_json(&Value::Array(report));
    } else {
        for e in &report {
            println!("{}: {}", e["agent"].as_str().unwrap_or(""), e["status"].as_str().unwrap_or(""));
            if let Some(why) = e["why"].as_str() {
                println!("  {why}");
            }
            for f in e["files"].as_array().into_iter().flatten() {
                let action = f["action"].as_str().unwrap_or("");
                let action =
                    if dry_run && action != "unchanged" { format!("would {action}") } else { action.to_string() };
                println!("  {action:<20} {}", f["path"].as_str().unwrap_or(""));
            }
            for n in e["notes"].as_array().into_iter().flatten() {
                println!("  {}", n.as_str().unwrap_or(""));
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
    fn the_stub_shares_the_guides_front_matter() {
        let s = stub();
        let front = |t: &str| t.split("\n---\n").next().map(str::to_string);
        assert_eq!(front(&s), front(GUIDE));
        assert!(s.lines().any(|l| l == MARKER));
        assert!(s.contains("leon skill show"));
        assert!(s.lines().any(|l| l == "name: leon"));
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
