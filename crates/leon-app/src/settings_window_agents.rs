//! The Settings window's Agents: leon's skill in each coding agent, as `leon skill status` says
//! (agent_skill.rs), with the buttons that run `leon skill setup|remove --agent X` (docs/agents.md).

use std::collections::HashMap;

use super::sections::matches;
use super::server::text_button;
use super::*;
use crate::agent_skill::{self, AgentRow, Verb};

/// The binary the rows' commands run, and what each agent's last command came to.
#[derive(Default)]
pub(super) struct AgentsUi {
    leon: Option<PathBuf>,
    /// By agent: running now, and the last command's notes or failure.
    busy: HashMap<String, Verb>,
    said: HashMap<String, Result<String, String>>,
}

impl AgentRow {
    pub(super) fn matches(&self, q: &str) -> bool {
        matches(q, &[&self.agent, self.name(), &self.words(), "agents skill"])
    }
}

impl SettingsWindow {
    /// `leon skill status`, once read.
    pub fn agents(&self) -> Option<Result<Vec<AgentRow>, String>> {
        self.agents.clone()
    }

    /// A command an agent's row runs now (tests).
    pub fn agent_busy(&self, agent: &str) -> Option<Verb> {
        self.agents_ui.busy.get(agent).copied()
    }

    /// What an agent's last command came to: its notes, or why it failed.
    pub fn agent_said(&self, agent: &str) -> Option<Result<String, String>> {
        self.agents_ui.said.get(agent).cloned()
    }

    /// Run `leon --json skill status` with the binary `leon`, off the UI thread.
    pub fn read_agents(&mut self, leon: PathBuf, cx: &mut Context<Self>) {
        self.agents = None;
        self.reread_agents(leon, cx);
    }

    /// `read_agents`, keeping the rows shown until the new ones come.
    fn reread_agents(&mut self, leon: PathBuf, cx: &mut Context<Self>) {
        self.agents_ui.leon = Some(leon.clone());
        cx.spawn(async move |this, cx| {
            let read = cx.background_executor().spawn(async move { agent_skill::status(&leon) }).await;
            this.update(cx, |sw, cx| {
                sw.agents = Some(read);
                cx.notify();
            })
        })
        .detach();
    }

    /// A row's button: `leon skill setup|remove --agent AGENT`, then the status again.
    fn run_agent(&mut self, verb: Verb, agent: String, cx: &mut Context<Self>) {
        let Some(leon) = self.agents_ui.leon.clone() else { return };
        if self.agents_ui.busy.contains_key(&agent) {
            return;
        }
        self.agents_ui.busy.insert(agent.clone(), verb);
        cx.notify();
        cx.spawn(async move |this, cx| {
            let (bin, who) = (leon.clone(), agent.clone());
            let done = cx.background_executor().spawn(async move { agent_skill::change(&bin, verb, &who) }).await;
            this.update(cx, |sw, cx| {
                sw.agents_ui.busy.remove(&agent);
                sw.agents_ui.said.insert(agent, done.map(|notes| notes.join(" ")));
                sw.reread_agents(leon, cx);
                cx.notify();
            })
        })
        .detach();
    }

    /// Agents' first line, and what came of reading the skill's state.
    pub(super) fn agents_intro(&self, t: &Theme) -> gpui::Div {
        let state = match &self.agents {
            None => Some(("Reading leon skill status…".to_string(), t.muted)),
            Some(Err(e)) => Some((e.clone(), t.error)),
            Some(Ok(_)) => None,
        };
        Self::label(
            t,
            "leon's skill teaches a coding agent to drive leon",
            "The same as leon skill status says (docs/agents.md). Leon.app uses CLAUDE_CONFIG_DIR and \
             CODEX_HOME only when it was started with them; if your shell sets them, run leon skill setup there.",
            false,
        )
        .children(state.map(|(s, c)| div().text_size(px(SECONDARY_TEXT)).text_color(rgb(c)).child(s)))
    }

    /// The agents at `rows`: each with its state, its paths, what setup writes, and its buttons;
    /// or, where a button would be wrong, the command that does it.
    pub(super) fn agent_rows(&self, rows: &[usize], t: &Theme, cx: &mut Context<Self>) -> gpui::Div {
        let mut out = div().flex().flex_col().gap(px(16.));
        let Some(Ok(list)) = &self.agents else { return out };
        let home = std::env::var_os("HOME").map(PathBuf::from);
        let small = |s: String, c: u32| div().text_size(px(SECONDARY_TEXT)).text_color(rgb(c)).child(s);
        for a in rows.iter().filter_map(|i| list.get(*i)) {
            let paths: Vec<String> = std::iter::once(&a.skill)
                .chain(&a.hook)
                .map(|p| home_relative(Path::new(p), home.as_deref()))
                .collect();
            let busy = self.agents_ui.busy.get(&a.agent);
            let mut buttons = div().flex().gap(px(8.));
            for verb in a.verbs() {
                let (agent, id) = (a.agent.clone(), format!("agent-{}-{}", a.agent, verb.label().to_lowercase()));
                buttons = buttons.child(text_button(t, id, verb.label(), cx, move |sw, _, cx| {
                    sw.run_agent(verb, agent.clone(), cx)
                }));
            }
            let words = match busy {
                Some(Verb::Install) => "Installing…".to_string(),
                Some(Verb::Update) => "Updating…".to_string(),
                Some(Verb::Remove) => "Removing…".to_string(),
                None => a.words(),
            };
            let ok = a.status == "current";
            let said = self.agents_ui.said.get(&a.agent).filter(|_| busy.is_none()).and_then(|r| match r {
                Ok(notes) if notes.is_empty() => None,
                Ok(notes) => Some(small(notes.clone(), t.muted)),
                Err(why) => Some(small(why.clone(), t.error)),
            });
            let command =
                a.setup.clone().filter(|_| a.verbs().is_empty()).map(|cmd| {
                    div().font_family("Menlo").text_size(px(SECONDARY_TEXT)).text_color(rgb(t.text)).child(cmd)
                });
            let sel = format!("agent-{}-said", a.agent);
            out = out.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(4.))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap(px(12.))
                            .child(div().text_color(rgb(t.title)).child(a.name().to_string()))
                            .child(div().text_color(rgb(if ok { t.done } else { t.muted })).child(words)),
                    )
                    .child(small(paths.join("  ·  "), t.muted))
                    .when(a.status != "current" && a.found, |d| d.child(small(a.writes(), t.muted)))
                    .children(command)
                    .when(busy.is_none() && !a.verbs().is_empty(), |d| d.child(buttons))
                    .children(said.map(|s| s.debug_selector(|| sel))),
            );
        }
        out
    }
}
