//! The Settings window's sections, the rows in each, the search over them, the sidebar and the
//! page the sidebar or the search shows.

use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Section {
    General,
    Appearance,
    Terminal,
    Notifications,
    Shortcuts,
    Agents,
    RemoteHosts,
    Server,
}

impl Section {
    /// In the sidebar's order.
    pub const ALL: [Section; 8] = [
        Section::General,
        Section::Appearance,
        Section::Terminal,
        Section::Notifications,
        Section::Shortcuts,
        Section::Agents,
        Section::RemoteHosts,
        Section::Server,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Section::General => "General",
            Section::Appearance => "Appearance",
            Section::Terminal => "Terminal",
            Section::Notifications => "Notifications",
            Section::Shortcuts => "Shortcuts",
            Section::Agents => "Agents",
            Section::RemoteHosts => "Remote Hosts",
            Section::Server => "Server",
        }
    }

    /// Its sidebar row's debug selector.
    fn selector(self) -> &'static str {
        match self {
            Section::General => "section-general",
            Section::Appearance => "section-appearance",
            Section::Terminal => "section-terminal",
            Section::Notifications => "section-notifications",
            Section::Shortcuts => "section-shortcuts",
            Section::Agents => "section-agents",
            Section::RemoteHosts => "section-hosts",
            Section::Server => "section-server",
        }
    }
}

/// A row of settings: its section, id, label and the Lua names it changes.
pub(super) struct Entry {
    pub section: Section,
    pub id: &'static str,
    pub label: &'static str,
    pub keys: &'static [&'static str],
}

/// A row on the page: a settings row, a shortcut or an agent (by index).
#[derive(Clone, Copy, PartialEq)]
pub(super) enum Item {
    Entry(&'static str),
    Shortcut(usize),
    Agent(usize),
    Host(usize),
}

const fn entry(section: Section, id: &'static str, label: &'static str, keys: &'static [&'static str]) -> Entry {
    Entry { section, id, label, keys }
}

use Section::*;

/// Every settings row, in the order each section shows them.
pub(super) const ENTRIES: [Entry; 16] = [
    entry(General, "resume", "Resume programs after a restart", &["resume"]),
    entry(General, "check_for_updates", "Look for updates once a day", &["check_for_updates"]),
    entry(Appearance, "theme", "Theme", &["theme"]),
    entry(Appearance, "translucency", "Translucency", &["translucency"]),
    entry(Appearance, "fade_inactive_panes", "Fade panes without the focus", &["fade_inactive_panes"]),
    entry(Appearance, "font", "Font", &["font.family", "font.size"]),
    entry(Terminal, "copy_on_select", "Copy text as I select it", &["copy_on_select"]),
    entry(Terminal, "repeat_time", "Repeat keys without the prefix", &["repeat_time"]),
    entry(Terminal, "hint_patterns", "Hint patterns", &["hint_patterns"]),
    entry(Notifications, "notifications.done", rows::NOTIFY_DONE, &["notifications.done"]),
    entry(Notifications, "notifications.blocked", rows::NOTIFY_BLOCKED, &["notifications.blocked"]),
    entry(Notifications, "notifications.error", rows::NOTIFY_ERROR, &["notifications.error"]),
    entry(Agents, "agents", "Coding agents", &["skill"]),
    entry(RemoteHosts, "hosts", "Remote hosts", &["hosts", "ssh", "add host", "tailnet"]),
    entry(Server, "server", "This Mac's server", &["server", "running", "restart", "stop", "socket", "log", "version"]),
    entry(
        Server,
        "tailscale",
        "Tailscale",
        &["tailscale", "tailnet", "serve", "serve_tailscale", "serve_tailscale_port", "serve_tailscale_tags"],
    ),
];

/// Whether the row `id` shows a setting from init.lua (blank until init.lua has been read).
fn from_init_lua(id: &str) -> bool {
    ENTRIES.iter().any(|e| e.id == id && e.keys.iter().any(|k| leon_lua::settings::KEYS.contains(k)))
}

/// Whether every word of `query` (lower case) is in one of `texts`.
pub(super) fn matches(query: &str, texts: &[&str]) -> bool {
    let hay = texts.join(" ").to_lowercase();
    query.split_whitespace().all(|w| hay.contains(w))
}

impl SettingsWindow {
    /// Every setting a control changes (each of `leon_lua::settings::KEYS` must be one).
    pub fn controls() -> Vec<&'static str> {
        ENTRIES.iter().flat_map(|e| e.keys).copied().filter(|k| leon_lua::settings::KEYS.contains(k)).collect()
    }

    pub fn section(&self) -> Section {
        self.section
    }

    pub fn query(&self) -> &str {
        &self.query
    }

    /// The ids of the rows the page shows, in order: a setting row's id, `shortcut:<action>`,
    /// `agent:<name>`.
    pub fn shown(&self) -> Vec<String> {
        let id = |item: &Item| match *item {
            Item::Entry(id) => id.to_string(),
            Item::Shortcut(i) => self.shortcuts[i].id(),
            Item::Agent(i) => match &self.agents {
                Some(Ok(list)) => format!("agent:{}", list[i].agent),
                _ => String::new(),
            },
            Item::Host(i) => format!("host:{}", self.hosts[i].label),
        };
        self.pages().into_iter().flat_map(|(_, items)| items).map(|i| id(&i)).collect()
    }

    /// What the page shows: the section, or with a query each section with a match, and its rows.
    fn pages(&self) -> Vec<(Section, Vec<Item>)> {
        let q = self.query.trim().to_lowercase();
        let pick = |s: Section| -> Vec<Item> {
            let any = q.is_empty();
            let mut items: Vec<Item> = ENTRIES
                .iter()
                .filter(|e| e.section == s && (any || matches(&q, &[e.label, s.name(), &e.keys.join(" ")])))
                .map(|e| Item::Entry(e.id))
                .collect();
            if s == Shortcuts {
                let hits = self.shortcuts.iter().enumerate().filter(|(_, r)| any || r.matches(&q));
                items.extend(hits.map(|(i, _)| Item::Shortcut(i)));
            }
            if s == Agents {
                if let Some(Ok(list)) = &self.agents {
                    let hits = list.iter().enumerate().filter(|(_, a)| any || a.matches(&q));
                    items.extend(hits.map(|(i, _)| Item::Agent(i)));
                }
            }
            if s == RemoteHosts {
                let hits = self.hosts.iter().enumerate().filter(|(_, h)| {
                    any || matches(&q, &[&h.label, &h.endpoint, h.tailnet.as_deref().unwrap_or(""), "hosts"])
                });
                items.extend(hits.map(|(i, _)| Item::Host(i)));
            }
            items
        };
        if q.is_empty() {
            return vec![(self.section, pick(self.section))];
        }
        Section::ALL.into_iter().map(|s| (s, pick(s))).filter(|(_, items)| !items.is_empty()).collect()
    }

    pub(super) fn sidebar(&self, t: &Theme, cx: &mut Context<Self>) -> gpui::Div {
        let searching = !self.query.trim().is_empty();
        let hits: Vec<Section> = if searching { self.pages().into_iter().map(|(s, _)| s).collect() } else { vec![] };
        let field = div()
            .id("settings-search")
            .debug_selector(|| "settings-search".into())
            .mx(px(10.))
            .mb(px(10.))
            .px(px(8.))
            .py(px(5.))
            .rounded(px(6.))
            .border_1()
            .border_color(rgb(t.border))
            .bg(rgb(t.bg))
            .flex()
            .items_center()
            .gap(px(6.))
            .child(Icon::Search.at(ROW_ICON, t.muted))
            .child(if self.query.is_empty() {
                div().text_color(rgb(t.muted)).child("Search")
            } else {
                div().text_color(rgb(t.text)).child(self.query.clone())
            })
            .on_click(cx.listener(|sw, _, window, _| window.focus(&sw.focus)));
        let mut list = div().flex().flex_col().gap(px(2.)).px(px(8.));
        for s in Section::ALL {
            let on = !searching && s == self.section;
            let dim = searching && !hits.contains(&s);
            list = list.child(
                div()
                    .id(s.selector())
                    .debug_selector(|| s.selector().into())
                    .px(px(10.))
                    .py(px(6.))
                    .rounded(px(6.))
                    .cursor_pointer()
                    .text_color(rgb(if dim { t.muted } else { t.text }))
                    .when(on, |d| d.bg(rgb(t.selected)).text_color(rgb(t.title)))
                    .when(!on, |d| d.hover(|d| d.bg(rgb(t.hover))))
                    .on_click(cx.listener(move |sw, _, window, cx| {
                        if s == Server {
                            sw.read_server(cx); // how it is now
                        }
                        sw.section = s;
                        sw.query.clear();
                        window.focus(&sw.focus);
                        cx.notify();
                    }))
                    .child(s.name()),
            );
        }
        div()
            .w(px(200.))
            .flex_none()
            .flex()
            .flex_col()
            .border_r_1()
            .border_color(rgb(t.border))
            .child(
                div()
                    .h(px(38.))
                    .flex_none()
                    .flex()
                    .items_center()
                    .pl(px(84.))
                    .text_color(rgb(t.title))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("Settings"),
            )
            .child(field)
            .child(list)
    }

    pub(super) fn page(&self, t: &Theme, cx: &mut Context<Self>) -> gpui::Div {
        let mut body = div().flex().flex_col().gap(px(28.));
        let pages = self.pages();
        if pages.is_empty() {
            return body.child(
                div()
                    .debug_selector(|| "no-results".into())
                    .pt(px(8.))
                    .text_color(rgb(t.muted))
                    .child(format!("No setting or shortcut matches “{}”", self.query.trim())),
            );
        }
        for (s, items) in pages {
            let mut part = div().flex().flex_col().gap(px(22.)).child(
                div()
                    .text_size(px(UI_TEXT + 6.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(t.title))
                    .child(s.name()),
            );
            if s == Shortcuts {
                part = part.child(self.shortcuts_intro(t, cx));
            }
            let (mut shortcuts, mut agents, mut hosts) = (vec![], vec![], vec![]);
            for item in items {
                match item {
                    // until init.lua reads, only the rows that don't come from it
                    Item::Entry(id) if self.loaded.is_some() || !from_init_lua(id) => {
                        part = part.child(self.entry(id, t, cx))
                    }
                    Item::Entry(_) => {}
                    Item::Shortcut(i) => shortcuts.push(i),
                    Item::Agent(i) => agents.push(i),
                    Item::Host(i) => hosts.push(i),
                }
            }
            if !hosts.is_empty() {
                part = part.child(self.host_rows(&hosts, t, cx));
            }
            if !shortcuts.is_empty() {
                part = part.child(self.shortcut_rows(&shortcuts, t));
            }
            if !agents.is_empty() {
                part = part.child(self.agent_rows(&agents, t));
            }
            body = body.child(part);
        }
        body
    }

    /// The control for a settings row.
    fn entry(&self, id: &str, t: &Theme, cx: &mut Context<Self>) -> gpui::AnyElement {
        let Some(&Entry { id, label, .. }) = ENTRIES.iter().find(|e| e.id == id) else {
            return div().into_any_element();
        };
        match id {
            "theme" => self.theme_section(t, cx).into_any_element(),
            "font" => self.font_section(t, cx).into_any_element(),
            "hint_patterns" => self.patterns(t, cx).into_any_element(),
            "agents" => self.agents_intro(t).into_any_element(),
            "hosts" => self.hosts_intro(t, cx).into_any_element(),
            "server" => self.server_page(t, cx).into_any_element(),
            "tailscale" => {
                self.tailscale_section(t, cx).debug_selector(|| "tailscale-section".into()).into_any_element()
            }
            "resume" => self.choice(t, "resume", label, rows::RESUME_NOTE, &rows::RESUME, cx).into_any_element(),
            "repeat_time" => self.stepper(t, "repeat_time", label, rows::REPEAT, cx).into_any_element(),
            "translucency" => self.stepper(t, "translucency", label, rows::TRANSLUCENCY, cx).into_any_element(),
            "fade_inactive_panes" => self.stepper(t, "fade_inactive_panes", label, rows::FADE, cx).into_any_element(),
            key => self.toggle(t, key, label, cx).into_any_element(),
        }
    }
}
