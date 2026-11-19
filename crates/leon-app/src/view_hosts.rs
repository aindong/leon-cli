//! The window's machines: which host the selection is on, each host's label in the sidebar (a dot
//! for its state, a note while it isn't connected), the stale overlay over a reconnecting host's
//! panes, New Session on a host, and the Add Host prompt in the palette (the tailnet's machines,
//! when Tailscale runs here, then a label and an endpoint).

use super::*;
use crate::hosts::Machine;
use crate::palette::Item;

#[path = "view_hosts_edit.rs"]
mod edit;

/// What the palette asks for.
#[derive(Clone)]
pub(super) enum Ask {
    /// Pick one of the tailnet's machines (the query searches them), or go on to a typed host.
    Tailnet,
    HostLabel,
    HostEndpoint {
        label: String,
    },
    /// A new name for a tab or a session (view_rows.rs); `name` is the one it has.
    Rename {
        row: super::sidebar::rows::RowRef,
        name: String,
    },
}

/// A machine as the sidebar shows it.
#[derive(Debug, Clone)]
pub struct HostRow {
    pub label: String,
    pub state: HostState,
    /// What the sidebar says under the label (`unreachable: …`), while not connected.
    pub note: Option<String>,
    pub collapsed: bool,
    pub sessions: Vec<HostSession>,
    /// `tailnet: LOGIN` for a machine on this machine's tailnet.
    pub tailnet: Option<String>,
}

#[derive(Debug, Clone)]
pub struct HostSession {
    pub id: String,
    pub title: String,
    pub tabs: Vec<String>,
}

impl Workspace {
    /// This Mac and each host, with their state and sessions.
    pub fn host_rows(&self) -> Vec<HostRow> {
        let rows = self.rows();
        let sessions = |h: usize| {
            let mine = rows.iter().filter(|r| r.host == h);
            let tabs = |r: &SessionRow| r.windows.iter().map(|w| w.title.clone()).collect();
            mine.map(|r| HostSession { id: r.id.clone(), title: r.title.clone(), tabs: tabs(r) }).collect()
        };
        let machines = self.hosts.machines().iter().enumerate();
        machines
            .map(|(h, m)| {
                let state = m.state();
                HostRow {
                    tailnet: self.tailnet_line(m),
                    note: state.note(&m.label),
                    state,
                    collapsed: self.host_collapsed(&m.label),
                    sessions: sessions(h),
                    label: m.label.clone(),
                }
            })
            .collect()
    }

    /// A host's remote, while it is connected or reconnecting.
    pub fn host_remote(&self, label: &str) -> Option<Rc<Remote>> {
        self.hosts.machines().get(self.hosts.index_of(label)?)?.remote().cloned()
    }

    /// The host a pane on screen is on, when it isn't This Mac (its header names it).
    pub fn pane_host(&self, block_id: &str) -> Option<String> {
        let m = &self.hosts.machines()[self.host];
        (!m.is_local() && self.app.model().block(block_id).is_some()).then(|| m.label.clone())
    }

    /// Whether the sidebar groups sessions under their machines: only once there is a host.
    pub(super) fn grouped(&self) -> bool {
        self.hosts.machines().len() > 1
    }

    pub(super) fn host_collapsed(&self, label: &str) -> bool {
        self.settings.collapsed_hosts.iter().any(|l| l == label)
    }

    /// Collapse or expand a machine's group; kept for the next launch.
    pub(super) fn toggle_host(&mut self, label: &str, cx: &mut Context<Self>) {
        let c = &mut self.settings.collapsed_hosts;
        match c.iter().position(|l| l == label) {
            Some(i) => drop(c.remove(i)),
            None => c.push(label.to_string()),
        }
        self.save_settings();
        cx.notify();
    }

    /// Act on machine `i` from now on; the panes of the one left stop streaming.
    pub(super) fn use_host(&mut self, i: usize) {
        let Some(remote) = self.hosts.machines().get(i).and_then(Machine::remote).cloned() else { return };
        if i != self.host {
            self.app.watch(Vec::new());
            (self.host, self.app) = (i, remote);
        }
    }

    /// On a wake: pick up hosts that connected or gave up. True when a host gained or lost its
    /// remote.
    pub(super) fn poll_hosts(&mut self) -> bool {
        let changed = self.hosts.poll();
        if changed {
            self.send_palette(); // a host that connected gets the theme's colours
        }
        if changed && !self.hosts.machines()[self.host].remote().is_some_and(|r| Rc::ptr_eq(r, &self.app)) {
            self.use_host(0); // the selected host went away
        }
        changed
    }

    /// After the models' commit: select a session made with New Session once its machine's model
    /// has it. True when it did.
    pub(super) fn select_made(&mut self) -> bool {
        let later = lock(&self.select_later).clone();
        if let Some((host, sid)) = later {
            let there = self.hosts.machines().get(host).and_then(Machine::remote);
            if there.is_some_and(|r| r.model().session(&sid).is_some()) {
                *lock(&self.select_later) = None;
                self.use_host(host);
                self.session_id = Some(sid);
                return true;
            }
        }
        false
    }

    /// Create a session on machine `host` (This Mac when unknown) and select it, at once on This
    /// Mac, once it shows over a link.
    pub(super) fn new_session_on(&mut self, host: usize, cx: &mut Context<Self>) {
        let Some(remote) = self.hosts.machines().get(host).and_then(Machine::remote).cloned() else { return };
        let (later, wake) = (self.select_later.clone(), self.wake.clone());
        remote.act_later("session.create", json!({}), move |r| {
            if let Some(sid) = r.ok().and_then(|r| r["session_id"].as_str().map(String::from)) {
                *lock(&later) = Some((host, sid));
                wake(); // over a link the refetch's wake came first
            }
        });
        self.poll_hosts();
        self.select_made();
        cx.notify();
    }

    /// The palette's machine rows: New Session on each connected machine (when there are hosts).
    pub(super) fn host_items(&self) -> Vec<Item> {
        if !self.grouped() {
            return Vec::new();
        }
        let on = self.hosts.remotes().map(|(i, _)| self.hosts.machines()[i].label.clone());
        on.map(|label| Item {
            name: "session.new".into(),
            title: format!("New Session on {label}"),
            keywords: vec![label.clone()],
            shortcut: None,
            args: Some(json!({ "host": label })),
        })
        .collect()
    }

    /// What the sidebar says about a host on the tailnet: who you are there.
    pub(super) fn tailnet_line(&self, m: &Machine) -> Option<String> {
        let tailnet = self.hosts.tailnet()?.ok()?;
        tailnet.machine_for(m.host.as_ref()?).map(|_| tailnet.identity())
    }

    /// A machine's label: a dot for its state, the name, and under it who you are on the tailnet,
    /// and a note while not connected.
    pub(super) fn host_label(&self, h: usize, m: &HostLabel, t: &Theme, cx: &mut Context<Self>) -> impl IntoElement {
        let dot = match m.state {
            HostState::Connected => t.done,
            HostState::Connecting | HostState::Reconnecting { .. } => t.working,
            HostState::NeedsLogin(_) => t.blocked,
            HostState::Unreachable(_) => t.error,
        };
        let (label, collapsed, sessions, state) = (m.label.clone(), m.collapsed, m.sessions, &m.state);
        div()
            .flex()
            .flex_col()
            .flex_none()
            .mt(px(if h == 0 { 2. } else { 10. }))
            .child(
                div()
                    .id(SharedString::from(format!("host-{h}")))
                    .debug_selector(|| sel(format!("host-{h}")))
                    .flex()
                    .items_center()
                    .gap(px(7.))
                    .h(px(24.))
                    .px(px(6.))
                    .rounded(px(5.))
                    .cursor_pointer()
                    .hover(|d| d.bg(rgb(t.hover)))
                    .text_size(px(SECONDARY_TEXT))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(t.muted))
                    .child(
                        div()
                            .debug_selector(|| sel(format!("host-dot-{h}")))
                            .flex_none()
                            .size(px(7.))
                            .rounded_full()
                            .bg(rgb(dot)),
                    )
                    .child(div().flex_1().min_w(px(0.)).line_clamp(1).text_ellipsis().child(label.clone()))
                    .when(collapsed, |d| {
                        d.children(sessions.map(|n| div().font_weight(FontWeight::NORMAL).child(n.to_string())))
                    })
                    .on_mouse_down(MouseButton::Left, cx.listener(move |ws, _, _, cx| ws.toggle_host(&label, cx))),
            )
            .children(m.tailnet.clone().map(|line| {
                div()
                    .debug_selector(|| sel(format!("host-tailnet-{h}")))
                    .pl(px(20.))
                    .pr(px(6.))
                    .text_size(px(SECONDARY_TEXT))
                    .text_color(rgb(t.muted))
                    .line_clamp(1)
                    .text_ellipsis()
                    .child(line)
            }))
            .children(state.note(&m.label).map(|note| {
                div()
                    .debug_selector(|| sel(format!("host-note-{h}")))
                    .pl(px(20.))
                    .pr(px(6.))
                    .pb(px(2.))
                    .text_size(px(SECONDARY_TEXT))
                    .text_color(rgb(if matches!(state, HostState::Unreachable(_)) { t.error } else { t.muted }))
                    .line_clamp(3)
                    .text_ellipsis()
                    .child(note)
            }))
    }

    /// Over the panes of a host whose link is down: they show what they last showed, dimmed, with
    /// the reconnect's progress.
    pub(super) fn render_stale(&self, window: &Window) -> Option<impl IntoElement> {
        let m = &self.hosts.machines()[self.host];
        let note = match m.state() {
            s @ HostState::Reconnecting { .. } => s.note(&m.label)?,
            _ => return None,
        };
        self.shown()?;
        let t = &self.theme;
        // the pane area is the window's bottom right corner (`pane_area`)
        let (w, h) = self.pane_area(window);
        let view = window.viewport_size();
        let (left, top) = (f32::from(view.width) - w, f32::from(view.height) - h);
        Some(
            div()
                .debug_selector(|| sel("stale-overlay".into()))
                .absolute()
                .left(px(left))
                .top(px(top))
                .w(px(w))
                .h(px(h))
                .flex()
                .items_end()
                .justify_center()
                .pb(px(28.))
                .bg(gpui::Hsla::from(rgb(t.chrome)).opacity(0.6))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .px(px(14.))
                        .py(px(8.))
                        .rounded(px(8.))
                        .bg(rgb(t.chrome))
                        .border_1()
                        .border_color(rgb(t.border))
                        .text_color(rgb(t.text))
                        .child(div().size(px(7.)).rounded_full().bg(rgb(t.working)))
                        .child(format!("{}: {note}", m.label)),
                ),
        )
    }

    /// Add Host…: the tailnet's machines when Tailscale is here (`{"tailnet": NAME}` adds one,
    /// `{"label": TEXT}` goes on to a typed host with that label), else the prompt for a label.
    pub(super) fn ask_host(&mut self, args: Option<&Value>, cx: &mut Context<Self>) {
        if let Some(name) = args.and_then(|a| a["tailnet"].as_str()) {
            let query = args.and_then(|a| a["query"].as_str()).unwrap_or_default().to_string();
            return self.add_tailnet(name, query, cx);
        }
        let typed = args.and_then(|a| a["label"].as_str()).map(String::from);
        let ask = match typed.is_none() && self.hosts.read_tailnet() {
            true => Ask::Tailnet,
            false => Ask::HostLabel,
        };
        let query = typed.clone().unwrap_or_default();
        self.palette =
            Some(Palette { query, selected: 0, ask: Some((ask, None)), switch: false, note: None, themes: None });
        if typed.is_some_and(|t| !t.is_empty()) {
            return self.ask_enter(cx); // the label typed into the tailnet's search
        }
        cx.notify();
    }

    /// The tailnet step's rows for the query: the machines that match, then a typed host.
    pub(super) fn tailnet_items(&self, query: &str) -> Vec<Item> {
        let tailnet = self.hosts.tailnet().and_then(Result::ok);
        let machines = tailnet.as_ref().map_or(&[][..], |t| &t.machines[..]);
        // searched by name only (an action's name would match every row)
        let rows: Vec<Item> = machines
            .iter()
            .map(|m| Item {
                name: m.dns.clone(),
                title: m.shown(),
                keywords: vec![],
                shortcut: Some(m.marks()),
                args: Some(json!({ "tailnet": m.dns, "query": query })),
            })
            .collect();
        let mut items: Vec<Item> = crate::palette::filter(&rows, query)
            .into_iter()
            .map(|i| Item { name: "client.host.add".into(), ..rows[i].clone() })
            .collect();
        items.push(Item {
            name: "client.host.add".into(),
            title: "Another host by its ssh address…".into(),
            keywords: vec![],
            shortcut: None,
            args: Some(json!({ "label": query.trim() })),
        });
        items
    }

    /// Add the tailnet's machine `name` as `leon hosts add --tailscale` does; a refusal reopens the
    /// step with the reason and the search it was picked from.
    fn add_tailnet(&mut self, name: &str, query: String, cx: &mut Context<Self>) {
        let added = (|| {
            let tailnet = self.hosts.tailnet().ok_or("no tailnet status yet")??;
            let m = tailnet.find(name)?;
            let host = tailnet.entry(m, None)?;
            if self.hosts.index_of(&host.label).is_some() {
                return Err(format!("{} {:?} already", leon_hosts::hosts::TAKEN, host.label));
            }
            let mut list = leon_hosts::hosts::load()?;
            leon_hosts::hosts::add(&mut list, host.clone()).and_then(|()| leon_hosts::hosts::save(&list))?;
            Ok::<_, String>(host)
        })();
        match added {
            Ok(host) => {
                self.palette = None;
                self.hosts.add(host);
            }
            Err(e) => {
                let ask = Some((Ask::Tailnet, Some(e)));
                self.palette = Some(Palette { query, selected: 0, ask, switch: false, note: None, themes: None });
            }
        }
        cx.notify();
    }

    /// The prompt's text and its one row (what to type, or why the last try was refused).
    pub(super) fn ask_rows(&self) -> Option<(String, String)> {
        let (ask, refused) = self.palette.as_ref()?.ask.as_ref()?;
        let (prompt, hint) = match ask {
            Ask::Tailnet => {
                let line = match self.hosts.tailnet() {
                    None => "Looking for your tailnet's machines…".to_string(),
                    Some(Ok(t)) if t.machines.is_empty() => "No other machines on your tailnet".to_string(),
                    Some(Ok(t)) => format!("Machines on your tailnet ({})", t.identity()),
                    Some(Err(e)) => e,
                };
                ("Search your tailnet…", line)
            }
            Ask::HostLabel => ("Host label…", "A label for the host: letters, digits, . _ - (devbox)".to_string()),
            Ask::HostEndpoint { label } => {
                ("ssh://[user@]host[:port]…", format!("How ssh reaches {label}: ssh://me@devbox.example.com"))
            }
            Ask::Rename { name, .. } => {
                let hint = "↩ sets it; empty, it shows its program's title again".to_string();
                return Some((format!("Rename “{name}” to…"), refused.clone().unwrap_or(hint)));
            }
        };
        Some((prompt.to_string(), refused.clone().unwrap_or(hint)))
    }

    /// Enter in the prompt: check what was typed as `leon hosts add` does, then ask the next
    /// thing, or save the host and start connecting to it.
    pub(super) fn ask_enter(&mut self, cx: &mut Context<Self>) {
        let Some(p) = self.palette.as_mut() else { return };
        let Some((ask, refused)) = p.ask.as_mut() else { return };
        let typed = p.query.trim().to_string();
        if let Ask::Rename { row, .. } = ask.clone() {
            self.palette = None;
            return self.rename_row(row, typed, cx);
        }
        let mut list = match leon_hosts::hosts::load() {
            Ok(l) => l,
            Err(e) => {
                *refused = Some(e);
                return cx.notify();
            }
        };
        match ask.clone() {
            Ask::Tailnet | Ask::Rename { .. } => {} // a row is picked like an action (palette_key)
            // one in the window but no longer in the file (removed from the CLI) is taken too
            Ask::HostLabel if self.hosts.index_of(&typed).is_some() => {
                *refused = Some(format!("{} {typed:?} already", leon_hosts::hosts::TAKEN));
            }
            Ask::HostLabel => match leon_hosts::hosts::check_new_label(&list, &typed) {
                Ok(()) => {
                    (*ask, *refused) = (Ask::HostEndpoint { label: typed }, None);
                    p.query.clear();
                }
                Err(e) => *refused = Some(e),
            },
            Ask::HostEndpoint { label } => {
                let host = leon_hosts::Host { label, endpoints: vec![typed], icon: None };
                let saved =
                    leon_hosts::hosts::add(&mut list, host.clone()).and_then(|()| leon_hosts::hosts::save(&list));
                match saved {
                    Ok(()) => {
                        self.palette = None;
                        self.hosts.add(host);
                    }
                    Err(e) => *refused = Some(e),
                }
            }
        }
        cx.notify();
    }

    /// The machine a `session.new` names (`{"host": LABEL}`), else the selected session's.
    pub(super) fn host_arg(&self, args: Option<&Value>) -> usize {
        let named = args.and_then(|a| a["host"].as_str()).and_then(|l| self.hosts.index_of(l));
        named.unwrap_or(self.host)
    }
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}
