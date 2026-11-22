//! The agent skill's notice (docs/agents.md): when Leon.app starts and an agent's skill is out of
//! date, "Agent skill out of date" with Update, once per app version (`updates.json` keeps it).
//! Update runs `leon skill setup --agent X` for each such agent (agent_skill.rs), off the UI
//! thread. It says nothing of a skill that is missing or someone else's.

use super::*;
use crate::agent_skill::{self, AgentRow, Verb};
use crate::updates::{Act, Notice};

#[derive(Default)]
pub(super) struct SkillUi {
    /// The check at the start has ended (tests).
    checked: bool,
    pub notice: Option<SkillNotice>,
}

pub(super) struct SkillNotice {
    leon: PathBuf,
    agents: Vec<AgentRow>,
    /// None: asking; Some(Ok(notes)) updated; Some(Err(why)) failed.
    done: Option<Result<String, String>>,
    working: bool,
}

impl Workspace {
    /// Ask `leon skill status` (with `leon`, the app's own binary) whether an agent's skill is out
    /// of date, and say so once for this version.
    pub fn check_agent_skill(&mut self, leon: PathBuf, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let bin = leon.clone();
            let rows = cx.background_executor().spawn(async move { agent_skill::status(&bin) }).await;
            this.update(cx, |ws, cx| {
                ws.skill.checked = true;
                let agents = rows.map(|r| agent_skill::outdated(&r)).unwrap_or_default();
                if !agents.is_empty() && ws.updates.skill_notice_once() {
                    ws.skill.notice = Some(SkillNotice { leon, agents, done: None, working: false });
                }
                cx.notify();
            })
        })
        .detach();
    }

    /// The start's check has ended (tests).
    pub fn agent_skill_checked(&self) -> bool {
        self.skill.checked
    }

    /// The text of the notice the window shows now, if any (tests).
    pub fn notice_text(&self) -> Option<String> {
        self.notice().map(|n| n.text)
    }

    pub(super) fn skill_notice(&self) -> Option<Notice> {
        let n = self.skill.notice.as_ref()?;
        let names: Vec<&str> = n.agents.iter().map(|a| a.name()).collect();
        let (text, actions) = match (&n.done, n.working) {
            (_, true) => ("Updating the agent skill…".to_string(), vec![]),
            (None, _) => {
                (format!("Agent skill out of date for {}.", names.join(" and ")), vec![Act::SkillUpdate, Act::Dismiss])
            }
            (Some(Ok(notes)), _) => {
                (format!("Agent skill updated. {notes}").trim_end().to_string(), vec![Act::Dismiss])
            }
            (Some(Err(why)), _) => (format!("The agent skill update failed: {why}"), vec![Act::Dismiss]),
        };
        Some(Notice { text, actions })
    }

    /// Update: `leon skill setup --agent X` for each agent the notice names, one after another.
    pub(super) fn update_agent_skill(&mut self, cx: &mut Context<Self>) {
        let Some(n) = self.skill.notice.as_mut().filter(|n| !n.working && n.done.is_none()) else { return };
        n.working = true;
        let (leon, agents) = (n.leon.clone(), n.agents.clone());
        cx.spawn(async move |this, cx| {
            let done = cx
                .background_executor()
                .spawn(async move {
                    let mut notes = vec![];
                    for a in &agents {
                        notes.extend(agent_skill::change(&leon, Verb::Update, &a.agent)?);
                    }
                    Ok(notes.join(" "))
                })
                .await;
            this.update(cx, |ws, cx| {
                if let Some(n) = ws.skill.notice.as_mut() {
                    (n.working, n.done) = (false, Some(done));
                }
                cx.notify();
            })
        })
        .detach();
    }
}
