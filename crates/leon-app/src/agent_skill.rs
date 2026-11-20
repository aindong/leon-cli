//! leon's skill in the coding agents (docs/agents.md), as the app shows and changes it: what
//! `leon --json skill status` says of each agent, and `leon --json skill setup|remove --agent X`,
//! run with the app's own binary (Leon.app runs as `leon app`). No GPUI here: Settings → Agents
//! (settings_window_agents.rs) and the out-of-date notice (view_skill.rs) call these off the UI
//! thread.

use std::path::Path;
use std::process::{Command, Stdio};

use serde_json::Value;

/// One agent's skill, from `leon skill status`.
#[derive(Debug, Clone, PartialEq)]
pub struct AgentRow {
    /// `claude` or `codex`.
    pub agent: String,
    /// current, outdated, missing, foreign or unreadable.
    pub status: String,
    /// Where its SKILL.md goes, and that file's own state.
    pub skill: String,
    pub skill_state: String,
    /// Where its hooks go and their state, when its config dir exists.
    pub hook: Option<String>,
    pub hook_state: Option<String>,
    /// The agent is on this machine (its config dir, or its program on PATH).
    pub found: bool,
    /// The command that sets it up, unless it is set up.
    pub setup: Option<String>,
}

/// What a row's button does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verb {
    Install,
    Update,
    Remove,
}

impl Verb {
    pub fn label(self) -> &'static str {
        match self {
            Verb::Install => "Install",
            Verb::Update => "Update",
            Verb::Remove => "Remove",
        }
    }

    /// The `leon skill` command it runs.
    pub fn command(self) -> &'static str {
        match self {
            Verb::Install | Verb::Update => "setup",
            Verb::Remove => "remove",
        }
    }
}

impl AgentRow {
    pub fn name(&self) -> &str {
        match self.agent.as_str() {
            "claude" => "Claude Code",
            "codex" => "Codex",
            other => other,
        }
    }

    /// The state in plain words.
    pub fn words(&self) -> String {
        match self.status.as_str() {
            "current" => "Up to date".into(),
            "outdated" if self.hook_state.as_deref().is_some_and(|s| s != "current") => {
                "Update available: new status hooks".into()
            }
            "outdated" => "Update available: a newer skill file".into(),
            "missing" if !self.found => format!("Not installed; {} isn't on this Mac", self.name()),
            "missing" => "Not installed".into(),
            "foreign" => "Changed by you: leon won't overwrite it".into(),
            "unreadable" => {
                let path = match self.skill_state.as_str() {
                    "unreadable" => &self.skill,
                    _ => self.hook.as_ref().unwrap_or(&self.skill),
                };
                format!("Can't read {path}")
            }
            other => other.into(),
        }
    }

    /// Its buttons. Never one that writes over a file leon didn't write, and no Install for an
    /// agent that isn't here: setup would make its config dir.
    pub fn verbs(&self) -> Vec<Verb> {
        match self.status.as_str() {
            "missing" if self.found => vec![Verb::Install],
            "outdated" => vec![Verb::Update, Verb::Remove],
            "current" => vec![Verb::Remove],
            _ => vec![],
        }
    }

    /// What setup writes, so a button is no surprise.
    pub fn writes(&self) -> String {
        let hooks = match (&self.hook, self.agent.as_str()) {
            (Some(p), _) => p.clone(),
            (None, "claude") => "Claude Code's settings.json".into(),
            (None, _) => "Codex's hooks.json".into(),
        };
        format!("Setup writes the skill file, plus leon's hooks in {hooks} (backed up once first).")
    }
}

/// `leon --json skill status`'s output, one row per agent.
pub fn parse_agents(out: &str) -> Result<Vec<AgentRow>, String> {
    let v: Value = serde_json::from_str(out).map_err(|e| format!("leon skill status said something else: {e}"))?;
    let list = v.as_array().ok_or("leon skill status didn't list agents")?;
    let text = |v: &Value| v.as_str().map(String::from);
    list.iter()
        .map(|a| {
            let agent = a["agent"].as_str().ok_or("an agent without a name")?.to_string();
            let status = a["status"].as_str().ok_or("an agent without a status")?.to_string();
            let setup = match status.as_str() {
                "current" => None,
                "foreign" => Some(format!("leon skill setup --agent {agent} --force")),
                _ => Some(format!("leon skill setup --agent {agent}")),
            };
            Ok(AgentRow {
                skill: text(&a["skill"]["path"]).unwrap_or_default(),
                skill_state: text(&a["skill"]["state"]).unwrap_or_default(),
                hook: text(&a["hook"]["path"]),
                hook_state: text(&a["hook"]["state"]),
                // an older leon's status has no `found`: its agents were there to list
                found: a["found"].as_bool().unwrap_or(true),
                agent,
                status,
                setup,
            })
        })
        .collect()
}

fn leon_json(leon: &Path, args: &[&str]) -> Result<(Value, String), String> {
    let out = Command::new(leon)
        .arg("--json")
        .args(args)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("can't run {}: {e}", leon.display()))?;
    let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
    let v = serde_json::from_slice(&out.stdout).map_err(|_| match err.is_empty() {
        true => format!("leon {} failed", args.join(" ")),
        false => err.clone(),
    })?;
    Ok((v, err))
}

/// `leon skill status`, run with `leon`.
pub fn status(leon: &Path) -> Result<Vec<AgentRow>, String> {
    let (v, _) = leon_json(leon, &["skill", "status"])?;
    parse_agents(&v.to_string())
}

/// `leon skill setup|remove --agent AGENT`: what it noted (Codex's /hooks), or why it failed.
pub fn change(leon: &Path, verb: Verb, agent: &str) -> Result<Vec<String>, String> {
    let (v, err) = leon_json(leon, &["skill", verb.command(), "--agent", agent])?;
    let e = &v[0];
    if let Some(why) = e["error"].as_str() {
        return Err(why.to_string());
    }
    if e["files"].as_array().into_iter().flatten().any(|f| f["action"] == "failed") {
        return Err(if err.is_empty() { "a file couldn't be written".into() } else { err });
    }
    Ok(e["notes"].as_array().into_iter().flatten().filter_map(|n| n.as_str().map(String::from)).collect())
}

/// The agents whose skill an update would bring up to date (not missing, not foreign).
pub fn outdated(rows: &[AgentRow]) -> Vec<AgentRow> {
    rows.iter().filter(|r| r.status == "outdated").cloned().collect()
}
