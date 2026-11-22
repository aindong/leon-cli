//! The Connect popover (`client.connect`, ⌘⇧K, the sidebar footer's left button): one place to
//! reach another machine. Your hosts with their state (choose one to go to it, or to try it again
//! now), the owner's other machines on the tailnet (from the local tailscaled's LocalAPI, read
//! only, `leon_hosts::peers`; one chosen is saved as `tailnet://NAME`), Add a host…, and this
//! Mac's web page (its address and QR code while served, else a way to Settings). It is the
//! popover component (popover.rs), owned as the row menu is (view_rows.rs).

use gpui::{ClipboardItem, Entity};
use leon_hosts::peers::Owned;
use leon_hosts::{Endpoint, Host};

use super::*;
use crate::popover::{self, Item, Popover, PopoverEvent};

pub(super) struct ConnectMenu {
    popover: Entity<Popover>,
    /// The web page's card (its address and QR code) rather than the list: the URL.
    web: Option<String>,
    _events: Subscription,
}

/// The popover's state, in `TermUi`.
#[derive(Default)]
pub(super) struct ConnectUi {
    menu: Option<ConnectMenu>,
    /// The owner's tailnet machines from the last LocalAPI read; None before one, or when there
    /// is no LocalAPI to ask.
    tailnet: Option<Vec<Owned>>,
    /// `server.status`'s `tailnet`, as last read (or as a test set it, which then stays).
    web: Option<Value>,
    web_pinned: bool,
}

/// What the popover says about a host's state.
fn state_word(s: &HostState) -> &'static str {
    match s {
        HostState::Connected => "connected",
        HostState::Connecting | HostState::Reconnecting { .. } => "connecting",
        HostState::Unreachable(_) => "unreachable",
        HostState::NeedsLogin(_) => "needs login",
    }
}

/// The address the web page is served at first, from `server.status`'s `tailnet`.
fn web_url(tailnet: Option<&Value>) -> Option<String> {
    let t = tailnet?;
    (t["state"] == "serving").then(|| t["web"].as_array()?.first()?.as_str().map(String::from)).flatten()
}

impl Workspace {
    /// The Connect popover, while it is open (its list or the web page's card).
    pub fn connect_menu(&self) -> Option<Entity<Popover>> {
        self.term.connect.menu.as_ref().map(|m| m.popover.clone())
    }

    /// Whether the Settings window is open.
    pub fn settings_open(&self) -> bool {
        self.settings_window.is_some()
    }

    /// Show the web page as if `server.status`'s `tailnet` said `tailnet` (tests, screenshots).
    pub fn show_web_status(&mut self, tailnet: Option<Value>, cx: &mut Context<Self>) {
        (self.term.connect.web, self.term.connect.web_pinned) = (tailnet, true);
        self.refresh_connect(cx);
    }

    /// `client.connect`: open the popover, or close it when it is open.
    pub(super) fn toggle_connect(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.term.connect.menu.take().is_some() {
            window.focus(&self.focus);
            return cx.notify();
        }
        self.open_connect(Vec::new(), None, window, cx);
        self.read_connect(cx);
    }

    /// Above the footer's Connect button, or under the titlebar's first button with the sidebar hidden.
    fn connect_anchor(&self, window: &Window) -> Bounds<Pixels> {
        let target = px(crate::icons::TOOLBAR_TARGET);
        match self.places().sidebar.is_some() {
            true => {
                let bottom = window.viewport_size().height;
                Bounds { origin: point(px(8.), bottom - px(30.)), size: size(target, px(30.)) }
            }
            false => Bounds { origin: point(px(TRAFFIC_LIGHTS), px(0.)), size: size(target, px(TITLEBAR)) },
        }
    }

    /// The popover with `lines` over its items, or the web page's card for `web`.
    fn open_connect(&mut self, lines: Vec<String>, web: Option<String>, window: &mut Window, cx: &mut Context<Self>) {
        let (anchor, theme) = (self.connect_anchor(window), self.theme);
        let items = match &web {
            Some(_) => vec![Item::new("copy", "Copy Address", None)],
            None => self.connect_items(),
        };
        let qr = web.clone();
        let p = cx.new(|cx| Popover::new(anchor, lines, items, theme, window, cx).with_qr(qr));
        let events =
            cx.subscribe_in(&p, window, |ws, _, ev: &PopoverEvent, window, cx| ws.connect_event(ev, window, cx));
        p.update(cx, |p, cx| p.focus(window, cx));
        self.term.connect.menu = Some(ConnectMenu { popover: p, web, _events: events });
        self.hide_preview(); // a tab's preview would cover it
        cx.notify();
    }

    /// Ask, off the main thread, for the tailnet's machines and for how the web page is served;
    /// each answer refreshes the list if it is still open.
    fn read_connect(&mut self, cx: &mut Context<Self>) {
        let peers = cx.background_executor().spawn(async { leon_hosts::peers::owned() });
        cx.spawn(async move |ws, cx| {
            let owned = peers.await.ok();
            let _ = ws.update(cx, |ws, cx| {
                ws.term.connect.tailnet = owned;
                ws.refresh_connect(cx);
            });
        })
        .detach();
        let (tx, rx) = futures::channel::oneshot::channel();
        self.hosts.local().spawn("server.status", json!({}), move |r| drop(tx.send(r)));
        cx.spawn(async move |ws, cx| {
            let tailnet = rx.await.ok().and_then(Result::ok).map(|s| s["tailnet"].clone());
            let _ = ws.update(cx, |ws, cx| {
                if !ws.term.connect.web_pinned {
                    ws.term.connect.web = tailnet;
                    ws.refresh_connect(cx);
                }
            });
        })
        .detach();
    }

    /// The open list again, from what is known now.
    fn refresh_connect(&mut self, cx: &mut Context<Self>) {
        let Some(m) = self.term.connect.menu.as_ref().filter(|m| m.web.is_none()) else { return };
        let (p, items) = (m.popover.clone(), self.connect_items());
        p.update(cx, |p, cx| p.set_items(items, cx));
    }

    /// Every host the window or hosts.json knows, This Mac aside.
    fn known_hosts(&self) -> Vec<Host> {
        let shown = self.hosts.machines().iter().filter_map(|m| m.host.clone());
        let mut all: Vec<Host> = shown.collect();
        let file = leon_hosts::hosts::load().unwrap_or_default();
        all.extend(file.into_iter().filter(|h| self.hosts.index_of(&h.label).is_none()));
        all
    }

    fn connect_items(&self) -> Vec<Item> {
        let mut out = Vec::new();
        let known = self.known_hosts();
        if !known.is_empty() {
            out.push(Item::heading("Your hosts"));
        }
        for h in &known {
            let state = self.hosts.index_of(&h.label).map(|i| self.hosts.machines()[i].state());
            let word = state.as_ref().map_or("not shown", state_word);
            out.push(Item::new(format!("host:{}", h.label), h.label.clone(), Some(word.into())));
        }
        // a machine that is already a host (by any of its endpoints) is listed there, not here
        let reaches = |o: &Owned| {
            let ends = known.iter().flat_map(|h| h.endpoints.iter().filter_map(|e| e.parse::<Endpoint>().ok()));
            let mut ends = ends.into_iter();
            ends.any(|e| {
                let at = e.host.trim_end_matches('.');
                at.eq_ignore_ascii_case(&o.dns) || at.eq_ignore_ascii_case(o.name())
            }) || known.iter().any(|h| h.label.eq_ignore_ascii_case(o.name()))
        };
        let owned: Vec<&Owned> = self.term.connect.tailnet.iter().flatten().filter(|o| !reaches(o)).collect();
        if !owned.is_empty() {
            out.push(Item::heading("On your tailnet"));
        }
        for o in owned {
            let hint = if o.online { "online" } else { "offline" };
            out.push(Item::new(format!("tailnet:{}", o.dns), o.name(), Some(hint.into())));
        }
        out.push(Item::new("add", "Add a host…", None));
        out.push(Item::heading("This Mac's web page"));
        out.push(match web_url(self.term.connect.web.as_ref()) {
            Some(url) => Item::new("web", "Open on another device", Some(url)),
            None => Item::new("web-settings", "Turn on in Settings…", None),
        });
        out
    }

    fn connect_event(&mut self, ev: &PopoverEvent, window: &mut Window, cx: &mut Context<Self>) {
        let Some(menu) = self.term.connect.menu.take() else { return };
        window.focus(&self.focus);
        cx.notify();
        let PopoverEvent::Chose(id) = ev else { return };
        if let Some(url) = menu.web {
            return cx.write_to_clipboard(ClipboardItem::new_string(url)); // Copy Address
        }
        match id.as_str() {
            "add" => self.ask_host(None, cx),
            "web-settings" => self.open_settings(cx),
            "web" => {
                let url = web_url(self.term.connect.web.as_ref()).unwrap_or_default();
                let note = "Open it on a phone or another machine on your Tailscale.".to_string();
                self.open_connect(vec![url.clone(), note], Some(url), window, cx);
            }
            id => {
                if let Some(label) = id.strip_prefix("host:") {
                    self.connect_host(label, cx);
                } else if let Some(dns) = id.strip_prefix("tailnet:") {
                    if let Err(e) = self.add_owned(dns) {
                        self.open_connect(vec![e], None, window, cx);
                    }
                }
            }
        }
    }

    /// A host chosen: go to it when it is connected, else try it now; one only in hosts.json
    /// joins the window.
    fn connect_host(&mut self, label: &str, cx: &mut Context<Self>) {
        let Some(i) = self.hosts.index_of(label) else {
            if let Some(h) = leon_hosts::hosts::load().unwrap_or_default().into_iter().find(|h| h.label == label) {
                self.hosts.add(h);
            }
            return cx.notify();
        };
        if self.hosts.machines()[i].remote().is_none() {
            self.hosts.retry_now(i);
            return cx.notify();
        }
        if self.host_collapsed(label) {
            self.toggle_host(label, cx);
        }
        if let Some(row) = self.rows().into_iter().find(|r| r.host == i) {
            self.select(&row.id, None, cx);
        }
    }

    /// Save the owner's machine `dns` as the host `tailnet://DNS`, under its short name, checked as
    /// `leon hosts add` checks one, and connect to it.
    fn add_owned(&mut self, dns: &str) -> Result<(), String> {
        let label = dns.split('.').next().unwrap_or_default().to_string();
        leon_hosts::hosts::check_label(&label)?;
        let endpoint: Endpoint = format!("tailnet://{dns}").parse()?;
        if self.hosts.index_of(&label).is_some() {
            return Err(format!("{} {label:?} already", leon_hosts::hosts::TAKEN));
        }
        let host = Host { label, endpoints: vec![endpoint.to_string()], icon: None };
        let mut list = leon_hosts::hosts::load()?;
        leon_hosts::hosts::add(&mut list, host.clone()).and_then(|()| leon_hosts::hosts::save(&list))?;
        self.hosts.add(host);
        Ok(())
    }

    /// The open Connect popover, over everything.
    pub(super) fn render_connect(&self, cx: &App) -> Option<impl IntoElement> {
        self.term.connect.menu.as_ref().map(|m| popover::show(&m.popover, cx))
    }
}
