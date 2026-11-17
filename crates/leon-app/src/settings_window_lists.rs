//! The Settings window's lists: the hint patterns (an editable list, each pattern checked before
//! it is written), the keymap's shortcuts (read-only) and the agents' skill (`leon skill status`).

use leon_core::hints::{compile, MAX_PATTERNS};

use super::sections::matches;
use super::*;
use crate::keymap::{keys_text, Keymap};

/// The hint pattern field.
pub(super) struct PatternField {
    focus: FocusHandle,
    draft: String,
    /// The pattern the draft replaces; None adds one.
    editing: Option<usize>,
    error: Option<String>,
    /// The field had the focus when the window last rendered.
    pub(super) typing: bool,
}

impl PatternField {
    pub(super) fn new(cx: &mut Context<SettingsWindow>) -> Self {
        PatternField { focus: cx.focus_handle(), draft: String::new(), editing: None, error: None, typing: false }
    }

    pub(super) fn focused(&self, window: &Window) -> bool {
        self.focus.is_focused(window)
    }
}

/// One binding, as Shortcuts lists it.
#[derive(Debug, Clone, PartialEq)]
pub struct ShortcutRow {
    /// As the README writes them (`⌘⇧D`, `⌃B C`).
    pub keys: String,
    pub title: String,
    /// None: a Lua function.
    pub action: Option<String>,
}

impl ShortcutRow {
    pub(super) fn id(&self) -> String {
        format!("shortcut:{}", self.action.as_deref().unwrap_or(&self.keys))
    }

    pub(super) fn matches(&self, q: &str) -> bool {
        matches(q, &[&self.keys, &self.title, self.action.as_deref().unwrap_or("lua function"), "shortcuts"])
    }
}

/// Every binding of `km`, titled as the palette titles its action.
fn shortcut_rows(km: &Keymap) -> Vec<ShortcutRow> {
    km.bindings()
        .iter()
        .map(|b| {
            let title = match (&b.action, &b.args) {
                (None, _) => "Lua function".to_string(),
                (Some(a), args) => {
                    let named = crate::palette::BUILTINS.iter().find(|(n, _)| n == a).map(|(_, t)| t.to_string());
                    let named = named.or_else(|| km.actions.iter().find(|c| c.name == *a).map(|c| c.title.clone()));
                    match (a.as_str(), args) {
                        ("client.tab.goto", Some(args)) => format!("Go to Tab {}", args["index"]),
                        (_, Some(args)) => format!("{} {args}", named.unwrap_or_else(|| a.clone())),
                        _ => named.unwrap_or_else(|| a.clone()),
                    }
                }
            };
            ShortcutRow { keys: keys_text(&b.seq), title, action: b.action.clone() }
        })
        .collect()
}

/// One agent's skill, from `leon skill status`.
#[derive(Debug, Clone, PartialEq)]
pub struct AgentRow {
    /// `claude` or `codex`.
    pub agent: String,
    /// current, outdated, missing, foreign or unreadable.
    pub status: String,
    /// Where its SKILL.md goes.
    pub skill: String,
    /// The command that sets it up, unless it is set up.
    pub setup: Option<String>,
}

impl AgentRow {
    fn name(&self) -> &str {
        match self.agent.as_str() {
            "claude" => "Claude Code",
            "codex" => "Codex",
            other => other,
        }
    }

    fn state(&self) -> &str {
        match self.status.as_str() {
            "current" => "Set up",
            "outdated" => "Set up by an older leon",
            "missing" => "Not set up",
            "foreign" => "Has a skill named leon that leon didn't install",
            "unreadable" => "Its files can't be read",
            other => other,
        }
    }

    pub(super) fn matches(&self, q: &str) -> bool {
        matches(q, &[&self.agent, self.name(), self.state(), "agents skill"])
    }
}

/// `leon --json skill status`'s output, one row per agent.
pub fn parse_agents(out: &str) -> Result<Vec<AgentRow>, String> {
    let v: Value = serde_json::from_str(out).map_err(|e| format!("leon skill status said something else: {e}"))?;
    let list = v.as_array().ok_or("leon skill status didn't list agents")?;
    list.iter()
        .map(|a| {
            let agent = a["agent"].as_str().ok_or("an agent without a name")?.to_string();
            let status = a["status"].as_str().ok_or("an agent without a status")?.to_string();
            let setup = (status != "current").then(|| format!("leon skill setup --agent {agent}"));
            Ok(AgentRow { skill: a["skill"]["path"].as_str().unwrap_or_default().to_string(), agent, status, setup })
        })
        .collect()
}

/// `path` as a `file://` URL, with what a URL can't hold percent-encoded.
fn file_url(path: &Path) -> String {
    let mut url = String::from("file://");
    for b in path.to_string_lossy().bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'/' | b'-' | b'_' | b'.' | b'~' => url.push(b as char),
            b => url += &format!("%{b:02X}"),
        }
    }
    url
}

impl SettingsWindow {
    /// The keymap's bindings, as Shortcuts lists them.
    pub fn shortcuts(&self) -> Vec<ShortcutRow> {
        self.shortcuts.clone()
    }

    /// `leon skill status`, once read.
    pub fn agents(&self) -> Option<Result<Vec<AgentRow>, String>> {
        self.agents.clone()
    }

    /// Why the last hint pattern wasn't added.
    pub fn pattern_error(&self) -> Option<String> {
        self.pattern.error.clone()
    }

    /// Ask the server for its keymap (init.lua's bindings over the defaults).
    pub(super) fn read_shortcuts(&mut self, cx: &mut Context<Self>) {
        let server = self.server.clone();
        cx.spawn(async move |this, cx| {
            let (tx, rx) = futures::channel::oneshot::channel();
            server.spawn("client.keymap", json!({}), move |r| drop(tx.send(r)));
            if let Ok(Ok(km)) = rx.await {
                let rows = shortcut_rows(&Keymap::app(&km));
                this.update(cx, |sw, cx| {
                    sw.shortcuts = rows;
                    cx.notify();
                })
                .ok();
            }
        })
        .detach();
    }

    /// Run `leon --json skill status` with the binary `leon`, off the UI thread.
    pub fn read_agents(&mut self, leon: PathBuf, cx: &mut Context<Self>) {
        self.agents = None;
        cx.spawn(async move |this, cx| {
            let read = cx
                .background_executor()
                .spawn(async move {
                    let out = std::process::Command::new(&leon)
                        .args(["--json", "skill", "status"])
                        .stdin(std::process::Stdio::null())
                        .output()
                        .map_err(|e| format!("can't run {}: {e}", leon.display()))?;
                    if !out.status.success() {
                        return Err(format!(
                            "leon skill status failed: {}",
                            String::from_utf8_lossy(&out.stderr).trim()
                        ));
                    }
                    parse_agents(&String::from_utf8_lossy(&out.stdout))
                })
                .await;
            this.update(cx, |sw, cx| {
                sw.agents = Some(read);
                cx.notify();
            })
        })
        .detach();
    }

    /// The hint pattern list: each pattern (click to edit it, × to remove it) and a field to add one.
    pub(super) fn patterns(&self, t: &Theme, cx: &mut Context<Self>) -> gpui::Div {
        let row = self.row("hint_patterns");
        let locked = Self::is_locked(&row);
        let list: Vec<String> = row
            .as_ref()
            .and_then(|r| r.value.as_array().cloned())
            .unwrap_or_default()
            .iter()
            .filter_map(|p| p.as_str().map(String::from))
            .collect();
        let note = "Regular expressions hints (⌘⇧H) label too, besides URLs, paths and hashes; at most 32";
        let mut out = div().flex().flex_col().gap(px(6.)).child(Self::label(t, "Hint patterns", note, locked));
        for (i, p) in list.iter().enumerate() {
            let (sel, editing) = (format!("pattern-{i}"), self.pattern.editing == Some(i));
            let text = p.clone();
            out = out.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        div()
                            .id(SharedString::from(sel.clone()))
                            .debug_selector(|| sel)
                            .flex_1()
                            .px(px(8.))
                            .py(px(4.))
                            .rounded(px(6.))
                            .font_family("Menlo")
                            .bg(rgb(if editing { t.selected } else { t.bg }))
                            .when(!locked, |d| {
                                d.cursor_pointer().on_click(cx.listener(move |sw, _, window, cx| {
                                    sw.pattern.draft = text.clone();
                                    sw.pattern.editing = Some(i);
                                    sw.pattern.error = None;
                                    window.focus(&sw.pattern.focus);
                                    cx.notify();
                                }))
                            })
                            .child(p.clone()),
                    )
                    .child(rows::button(t, format!("pattern-remove-{i}"), "×", !locked, cx, {
                        let list = list.clone();
                        move |sw, cx| {
                            let mut rest = list.clone();
                            rest.remove(i);
                            sw.pattern.editing = None;
                            sw.choose("hint_patterns", json!(rest), cx);
                        }
                    })),
            );
        }
        if !locked {
            let f = &self.pattern;
            let shown = if f.draft.is_empty() && !f.typing {
                div().text_color(rgb(t.muted)).child("Add a pattern, such as [A-Z]+-\\d+")
            } else {
                div().child(format!("{}{}", f.draft, if f.typing { "▏" } else { "" }))
            };
            out = out.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        div()
                            .id("pattern-field")
                            .debug_selector(|| "pattern-field".into())
                            .track_focus(&f.focus)
                            .on_key_down(cx.listener(Self::pattern_key))
                            .flex_1()
                            .px(px(8.))
                            .py(px(4.))
                            .rounded(px(6.))
                            .border_1()
                            .border_color(rgb(if f.typing { t.accent } else { t.border }))
                            .bg(rgb(t.bg))
                            .font_family("Menlo")
                            .cursor_text()
                            .child(shown),
                    )
                    .child(
                        div()
                            .id("pattern-add")
                            .debug_selector(|| "pattern-add".into())
                            .px(px(10.))
                            .py(px(4.))
                            .rounded(px(6.))
                            .border_1()
                            .border_color(rgb(t.border))
                            .cursor_pointer()
                            .hover(|d| d.bg(rgb(t.hover)))
                            .on_click(cx.listener(|sw, _, _, cx| sw.add_pattern(cx)))
                            .child(if f.editing.is_some() { "Save" } else { "Add" }),
                    ),
            );
            if let Some(why) = &f.error {
                out = out.child(
                    div()
                        .debug_selector(|| "pattern-error".into())
                        .text_size(px(SECONDARY_TEXT))
                        .text_color(rgb(t.error))
                        .child(why.clone()),
                );
            }
        }
        out.children(Self::locked(t, "hint_patterns", &row))
    }

    /// A key in the hint pattern field. ⌘ keys go on to the window (⌘W).
    fn pattern_key(&mut self, e: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let k = &e.keystroke;
        let m = &k.modifiers;
        if m.platform {
            return;
        }
        let f = &mut self.pattern;
        match k.key.as_str() {
            "enter" => self.add_pattern(cx),
            "escape" => {
                (f.draft, f.editing, f.error) = (String::new(), None, None);
                window.focus(&self.focus);
            }
            "backspace" => {
                f.draft.pop();
            }
            "space" => f.draft.push(' '),
            _ => match k.key_char.as_deref().filter(|c| !c.is_empty() && !m.control && !m.alt) {
                Some(typed) => f.draft.extend(typed.chars().filter(|c| !c.is_control())),
                None => return,
            },
        }
        cx.stop_propagation();
        cx.notify();
    }

    /// Add the field's pattern (or save the one it edits), once it compiles and the list has room;
    /// otherwise say why under the field and write nothing.
    fn add_pattern(&mut self, cx: &mut Context<Self>) {
        let Some(row) = self.row("hint_patterns").filter(|r| r.read_only.is_none()) else { return };
        let mut list: Vec<Value> = row.value.as_array().cloned().unwrap_or_default();
        let f = &mut self.pattern;
        let draft = f.draft.clone();
        if let Err(why) = compile(&draft) {
            f.error = Some(why);
            return cx.notify();
        }
        match f.editing.filter(|i| *i < list.len()) {
            Some(i) => list[i] = json!(draft),
            None if list.len() >= MAX_PATTERNS => {
                f.error = Some(format!("at most {MAX_PATTERNS} hint patterns; remove one first"));
                return cx.notify();
            }
            None => list.push(json!(draft)),
        }
        (f.draft, f.editing, f.error) = (String::new(), None, None);
        self.choose("hint_patterns", Value::Array(list), cx);
    }

    /// Shortcuts' first line, with the button that opens init.lua.
    pub(super) fn shortcuts_intro(&self, t: &Theme, cx: &mut Context<Self>) -> gpui::Div {
        div()
            .flex()
            .items_center()
            .justify_between()
            .gap(px(16.))
            .child(Self::label(
                t,
                "Every key the app binds: leon's own, and init.lua's on top",
                "Change them with leon.bind in init.lua (docs/lua/keys.md)",
                false,
            ))
            .child(
                div()
                    .id("edit-init-lua")
                    .debug_selector(|| "edit-init-lua".into())
                    .flex_none()
                    .px(px(10.))
                    .py(px(4.))
                    .rounded(px(6.))
                    .border_1()
                    .border_color(rgb(t.border))
                    .cursor_pointer()
                    .hover(|d| d.bg(rgb(t.hover)))
                    .on_click(cx.listener(|sw, _, _, cx| {
                        // an editor can't open a file that isn't there: start an empty one
                        if let Err(e) = std::fs::OpenOptions::new().append(true).create(true).open(&sw.config) {
                            sw.error = Some(format!("can't make {}: {e}", sw.config.display()));
                        }
                        cx.open_url(&file_url(&sw.config));
                    }))
                    .child("Edit in init.lua"),
            )
    }

    /// The shortcuts among `ids`, one line each: keys, title and action.
    // shortcut: plain rows, not a uniform_list; the keymap is a few dozen bindings. Upgrade if a
    // config binds hundreds.
    pub(super) fn shortcut_rows(&self, ids: &[String], t: &Theme) -> gpui::Div {
        let mut out = div().flex().flex_col();
        for r in self.shortcuts.iter().filter(|r| ids.contains(&r.id())) {
            out = out.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(16.))
                    .py(px(4.))
                    .border_b_1()
                    .border_color(rgb(t.border))
                    .child(div().w(px(110.)).flex_none().text_color(rgb(t.title)).child(r.keys.clone()))
                    .child(div().flex_1().child(r.title.clone()))
                    .child(
                        div()
                            .text_size(px(SECONDARY_TEXT))
                            .text_color(rgb(t.muted))
                            .font_family("Menlo")
                            .child(r.action.clone().unwrap_or_else(|| "Lua function".into())),
                    ),
            );
        }
        out
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
            "The same as leon skill status says; leon skill setup sets an agent up (docs/agents.md)",
            false,
        )
        .children(state.map(|(s, c)| div().text_size(px(SECONDARY_TEXT)).text_color(rgb(c)).child(s)))
    }

    /// The agents named in `names`: each with its state and, unless it is set up, the command that does.
    pub(super) fn agent_rows(&self, names: &[String], t: &Theme) -> gpui::Div {
        let mut out = div().flex().flex_col().gap(px(12.));
        let Some(Ok(list)) = &self.agents else { return out };
        let home = std::env::var_os("HOME").map(PathBuf::from);
        for a in list.iter().filter(|a| names.contains(&a.agent)) {
            let ok = a.setup.is_none();
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
                            .child(div().text_color(rgb(t.title)).child(a.name().to_string()))
                            .child(
                                div().text_color(rgb(if ok { t.done } else { t.muted })).child(a.state().to_string()),
                            ),
                    )
                    .child(
                        div()
                            .text_size(px(SECONDARY_TEXT))
                            .text_color(rgb(t.muted))
                            .child(home_relative(Path::new(&a.skill), home.as_deref())),
                    )
                    .children(a.setup.clone().map(|cmd| {
                        div().font_family("Menlo").text_size(px(SECONDARY_TEXT)).text_color(rgb(t.text)).child(cmd)
                    })),
            );
        }
        out
    }
}
