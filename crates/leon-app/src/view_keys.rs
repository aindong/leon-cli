//! Keys and actions: every keystroke goes through the keymap (defaults under the config's
//! bindings), the palette when it is open, else the focused pane. Actions run by name, from a key,
//! the palette or a menu.

use super::*;
use crate::palette::{self, Item};

impl Workspace {
    pub(super) fn key_down(&mut self, ev: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.stopped {
            return; // nothing to type into: the notice's buttons and ⌘Q are what's left
        }
        self.close_hover_card(window, cx);
        let ks = &match (self.native_key)() {
            Some((code, text)) => crate::input::retext(ev.keystroke.clone(), code, &text),
            None => ev.keystroke.clone(),
        };
        let chord = keymap::chord(ks);
        // every key closes the repeat window; only a repeatable one opens it again
        let mut repeat = std::mem::take(&mut self.repeat);
        if self.mode_key(ks, chord.as_ref(), cx) {
            cx.stop_propagation();
            return;
        }
        // a key binding syntax has no name for continues no sequence either: it ends it
        if chord.is_none() && !self.pending.is_empty() {
            self.pending.clear();
            cx.notify();
            cx.stop_propagation();
            return;
        }
        if let Some(c) = chord.as_ref().filter(|_| self.pending.is_empty()) {
            // GPUI's clock: the wall clock in the app, the test clock in a test (ADR R10)
            let now = cx.background_executor().now();
            if let Some((b, prefix)) = repeat.press(c, now, |p, k| self.keymap.repeatable(p, k)) {
                self.repeat.ran(&prefix, b.repeats, self.keymap.repeat_time, now);
                self.run_binding(b, prefix, c, window, cx);
                cx.stop_propagation();
                return;
            }
        }
        if let Some(c) = &chord {
            let escape = c.key == leon_lua::keys::Key::Named("escape") && c.mods == Default::default();
            if !self.pending.is_empty() && escape {
                self.pending.clear();
                cx.notify();
                cx.stop_propagation();
                return;
            }
            match self.keymap.press(&self.pending, c) {
                Pressed::Pending => {
                    self.pending.push(c.clone());
                    cx.notify();
                    cx.stop_propagation();
                    return;
                }
                Pressed::Run(b) => {
                    let prefix = std::mem::take(&mut self.pending);
                    self.repeat.ran(&prefix, b.repeats, self.keymap.repeat_time, cx.background_executor().now());
                    self.run_binding(b, prefix, c, window, cx);
                    cx.stop_propagation();
                    return;
                }
                // a key that continues no sequence ends it, and is swallowed (tmux style)
                Pressed::Unbound if !self.pending.is_empty() => {
                    self.pending.clear();
                    cx.notify();
                    cx.stop_propagation();
                    return;
                }
                Pressed::Unbound => {}
            }
        }
        let Some((sid, id, f)) = self.target_screen() else { return };
        if let Some(bytes) = crate::input::key_bytes(ks, f.app_cursor) {
            // only a key that reaches the pane is timed (LEON_FRAME_LOG): its echo moves the cursor
            if self.stats.borrow().logging() {
                if let Some(stamped) = crate::input::current_event_stamp() {
                    let cursor = self.drawn(&id).frame.cursor.map(|c| (c.line, c.col));
                    self.stats.borrow_mut().key(stamped, id.clone(), cursor);
                }
            }
            self.app.write(&sid, &id, &bytes);
            if self.view_top(&id).is_some() {
                self.scroll_to(&id, None); // typing goes back to the prompt
                cx.notify();
            }
            cx.stop_propagation();
        }
    }

    /// A binding's action runs here when it is the app's own; anything else (a config action, a
    /// Lua function) is the server's, through `client.key` so its fallthrough rules apply.
    fn run_binding(&mut self, b: Bound, prefix: Vec<Chord>, last: &Chord, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(name) = b.action.as_deref().filter(|n| n.contains('.')) {
            self.run_action(name, b.args.as_ref(), "key", window, cx);
            return;
        }
        let Some((sid, block)) = self.local_target() else { return };
        let prefix = (!prefix.is_empty()).then(|| prefix.iter().map(Chord::to_string).collect::<Vec<_>>().join(">"));
        let p = json!({ "session_id": sid, "block_id": block, "key": last.to_string(), "prefix": prefix });
        self.hosts.local().spawn("client.key", p, |r| report("a key binding", r));
    }

    /// Run an action by name: the app's own (`client.*`, `pane.*`, `session.*`) here, a config
    /// action on the server without waiting for it. An unknown app action is logged.
    pub fn run_action(
        &mut self,
        name: &str,
        args: Option<&Value>,
        origin: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match name {
            "session.new" => self.new_session_on(self.host_arg(args), cx),
            "session.switch" => self.switch_action(args, cx),
            "client.host.add" => self.ask_host(args, cx),
            "client.theme.change" => self.theme_action(args, cx),
            "client.tab.new" => self.new_tab(&NewTab, window, cx),
            "client.tab.close" | "session.close" | "client.tab.rename" | "session.rename" => {
                self.row_action(name, window, cx)
            }
            "client.tab.next" => self.step_tab(1, cx),
            "client.tab.previous" => self.step_tab(-1, cx),
            "client.tab.goto" => {
                if let Some(i) = args.and_then(|a| a["index"].as_u64()).filter(|i| *i >= 1) {
                    self.focus_window(i as usize - 1, cx);
                }
            }
            "pane.split.right" => self.split(Direction::Horizontal, "after", cx),
            "pane.split.down" => self.split(Direction::Vertical, "after", cx),
            "pane.split.left" => self.split(Direction::Horizontal, "before", cx),
            "pane.split.up" => self.split(Direction::Vertical, "before", cx),
            "pane.split" => {
                let way = args.and_then(|a| a["direction"].as_str()).unwrap_or("right");
                let (direction, side) = match way {
                    "left" => (Direction::Horizontal, "before"),
                    "up" => (Direction::Vertical, "before"),
                    "down" => (Direction::Vertical, "after"),
                    _ => (Direction::Horizontal, "after"),
                };
                self.split(direction, side, cx);
            }
            "pane.focus.left" | "pane.focus.right" | "pane.focus.up" | "pane.focus.down" => {
                self.focus_toward(&name["pane.focus.".len()..], cx)
            }
            "pane.zoom" => self.toggle_zoom(&ToggleZoom, window, cx),
            "pane.float"
            | "pane.new_floating"
            | "pane.unfloat"
            | "pane.scratch.toggle"
            | "pane.dock.left"
            | "pane.dock.right"
            | "pane.dock.bottom" => self.overlay_action(name, cx),
            "pane.close" => self.close_pane(&ClosePane, window, cx),
            "pane.paste" => self.paste(&Paste, window, cx),
            "client.sidebar.toggle" => self.toggle_sidebar(&ToggleSidebar, window, cx),
            "client.sidebar.activity.toggle" => {
                self.activity = !self.activity;
                cx.notify();
            }
            "client.overview.toggle" | "client.overview.session.toggle" => self.toggle_overview(name, cx),
            "client.tab_style.toggle" => self.toggle_tab_style(cx),
            "client.settings" => self.open_settings(cx),
            "client.pane_header.reveal" => self.toggle_pane_headers(cx),
            "client.font.increase" => self.set_font_size(self.font_size + 1., cx),
            "client.font.decrease" => self.set_font_size(self.font_size - 1., cx),
            "client.font.reset" => self.set_font_size(self.look.font_size, cx),
            "client.palette.toggle" => {
                self.palette = match self.palette {
                    Some(_) => None,
                    None => Some(Palette {
                        query: String::new(),
                        selected: 0,
                        ask: None,
                        switch: false,
                        note: None,
                        themes: None,
                    }),
                };
                cx.notify();
            }
            "pane.copy" => {
                if let Some((_, id, _)) = self.target() {
                    self.copy(&id, cx);
                }
            }
            "pane.select_all" => {
                if let Some((_, id, _)) = self.target() {
                    self.select_all(&id, cx);
                }
            }
            "client.find.open" | "client.find.next" | "client.find.previous" => self.find_action(name, cx),
            "client.copy_mode" | "client.hints" => self.mode_action(name, cx),
            "client.scroll.page_up" | "client.scroll.page_down" | "client.scroll.top" | "client.scroll.bottom" => {
                self.scroll_action(name, cx)
            }
            "client.update.check" => self.updates.check_now(),
            "client.update.install" => self.updates.install(),
            "client.update.restart" => self.restart_for_update(cx),
            n if n.starts_with("client.update.") => self.update_named(n, window, cx),
            "client.config.reload" => self.reload_config(),
            n if !n.contains('.') => {
                let (sid, block) = self.local_target().map(|(s, b)| (Some(s), Some(b))).unwrap_or_default();
                let p = json!({ "action": n, "args": args, "origin": origin, "session_id": sid, "block_id": block });
                self.hosts.local().spawn("client.action", p, |r| report("a config action", r));
            }
            other => eprintln!("leon: the app has no action {other}"),
        }
    }

    /// Have This Mac's server load init.lua again; the poll then asks for the new keymap and
    /// settings at once.
    pub(super) fn reload_config(&mut self) {
        let (inbox, wake) = (self.inbox.clone(), self.wake.clone());
        self.hosts.local().spawn("server.config_reload", json!({}), move |r| {
            report("the config reload", r);
            lock(&inbox).asking = false;
            wake(); // the poll asks for the new keymap
        });
        self.keymap_asked = None;
    }

    /// The config's actions run on This Mac's server (the keymap is This Mac's), so they get the
    /// target pane only when it is there. shortcut: a Lua binding can't reach a host's pane yet.
    fn local_target(&self) -> Option<(String, String)> {
        let (sid, block, _) = self.target()?;
        (self.host == 0).then_some((sid, block))
    }

    /// Every action the palette offers, with its shortcut, then New Session on each machine.
    fn palette_items(&self) -> Vec<Item> {
        let builtins = palette::BUILTINS.iter().map(|(n, t)| (n.to_string(), t.to_string(), Vec::new()));
        let config = self.keymap.actions.iter().map(|a| (a.name.clone(), a.title.clone(), a.keywords.clone()));
        let mut items: Vec<Item> = builtins
            .chain(config)
            .map(|(name, title, keywords)| Item {
                shortcut: self.keymap.shortcut(&name),
                name,
                title,
                keywords,
                args: None,
            })
            .collect();
        items.extend(self.host_items());
        items.extend(self.update_items());
        items
    }

    /// The open palette's rows, filtered: (title, shortcut). While it asks for text (Add Host…),
    /// its one row: what to type, or why the last try was refused.
    pub fn palette_rows(&self) -> Vec<(String, Option<String>)> {
        if let Some((_, row)) = self.ask_rows() {
            let picks = self.palette_matches().into_iter().map(|it| (it.title, it.shortcut));
            return picks.chain([(row, None)]).collect();
        }
        self.palette_matches().into_iter().map(|it| (it.title, it.shortcut)).collect()
    }

    /// The palette's rows for `query`, as if typed into it: (title, shortcut).
    pub fn palette_rows_for(&self, query: &str) -> Vec<(String, Option<String>)> {
        let items = self.palette_items();
        palette::filter(&items, query)
            .into_iter()
            .map(|i| (items[i].title.clone(), items[i].shortcut.clone()))
            .collect()
    }

    pub(super) fn palette_matches(&self) -> Vec<Item> {
        let Some(p) = self.palette.as_ref() else { return Vec::new() };
        match &p.ask {
            Some((hosts_view::Ask::Tailnet, _)) => return self.tailnet_items(&p.query),
            Some(_) => return Vec::new(),
            None => {}
        }
        if p.switch {
            return self.switcher_items(&p.query);
        }
        if let Some(list) = &p.themes {
            return self.theme_items(list, &p.query);
        }
        let items = self.palette_items();
        palette::filter(&items, &p.query).into_iter().map(|i| items[i].clone()).collect()
    }

    pub fn palette_open(&self) -> bool {
        self.palette.is_some()
    }

    pub(super) fn palette_selected(&self) -> usize {
        self.palette.as_ref().map_or(0, |p| p.selected)
    }

    /// What the palette's field says while nothing is typed.
    pub fn palette_prompt(&self) -> String {
        if let Some((prompt, _)) = self.ask_rows() {
            return prompt;
        }
        match self.palette.as_ref() {
            Some(p) if p.themes.is_some() => self.theme_prompt(),
            Some(p) if p.switch => "Go to a session, tab or block…",
            _ => "Type an action…",
        }
        .to_string()
    }

    pub fn palette_query(&self) -> String {
        self.palette.as_ref().map(|p| p.query.clone()).unwrap_or_default()
    }

    /// Keys while the palette is open: they edit the query and pick a row, never reach a pane.
    pub(super) fn palette_key(
        &mut self,
        ks: &gpui::Keystroke,
        chord: Option<&Chord>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let m = &ks.modifiers;
        let named = |name: &str| ks.key == name && !m.platform && !m.control;
        let bound = chord.and_then(|c| match self.keymap.press(&[], c) {
            Pressed::Run(b) => b.action,
            _ => None,
        });
        // the key of the list on screen closes it; the other one's key changes to that list
        let want_switch = match bound.as_deref() {
            Some("session.switch") => Some(true),
            Some("client.palette.toggle") => Some(false),
            _ => None,
        };
        // the theme list is neither: ⌘⇧P goes to the actions, its own key (⌘⇧T) closes it
        let themes = self.palette.as_ref().is_some_and(|p| p.themes.is_some());
        let toggle = want_switch.is_some_and(|w| self.palette.as_ref().is_some_and(|p| p.switch == w && !themes))
            || (themes && bound.as_deref() == Some("client.theme.change"));
        if let (Some(w), false, Some(p)) = (want_switch, toggle, self.palette.as_mut()) {
            (p.switch, p.query, p.selected, p.ask, p.note, p.themes) = (w, String::new(), 0, None, None, None);
            cx.notify();
            return;
        }
        let matches = self.palette_matches();
        let Some(p) = self.palette.as_mut() else { return };
        if named("escape") || toggle {
            self.palette = None;
        } else if named("enter") && p.ask.as_ref().is_some_and(|(a, _)| !matches!(a, hosts_view::Ask::Tailnet)) {
            self.ask_enter(cx);
        } else if named("enter") || (ks.key == "enter" && m.platform && p.switch) {
            // the list may have shrunk under the selection (the tailnet's status came in)
            let last = matches.len().saturating_sub(1);
            let pick = matches.into_iter().nth(p.selected.min(last));
            if !(m.platform && p.switch) || pick.is_none() {
                self.palette = None; // ⌘↩ in the switcher: session.switch closes it, or notes why not
            }
            if let Some(mut item) = pick {
                if m.platform {
                    item.args.get_or_insert_with(|| json!({}))["split"] = json!(true);
                    // ⌘↩: in a split
                }
                self.run_action(&item.name, item.args.as_ref(), "palette", window, cx);
            }
        } else if named("down") || (m.control && ks.key == "n") {
            p.selected = (p.selected + 1).min(matches.len().saturating_sub(1));
        } else if named("up") || (m.control && ks.key == "p") {
            p.selected = p.selected.saturating_sub(1);
        } else if named("backspace") {
            p.query.pop();
            (p.selected, p.note) = (0, None);
        } else if let Some(typed) = ks.key_char.as_deref().filter(|_| !m.platform && !m.control) {
            p.query.extend(typed.chars().filter(|c| !c.is_control()));
            (p.selected, p.note) = (0, None);
        } else if ks.key == "space" {
            p.query.push(' ');
            p.note = None;
        }
        cx.notify();
    }

    /// The pending sequence and what may follow it: (prefix, [(next key, what it runs)]).
    pub fn pending_hint(&self) -> Option<(String, Vec<(String, String)>)> {
        if self.pending.is_empty() {
            return None;
        }
        let prefix = self.pending.iter().map(Chord::to_string).collect::<Vec<_>>().join(">");
        Some((prefix, self.keymap.hints(&self.pending)))
    }
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// A failed call made without waiting has nobody to answer, so it goes to the log.
fn report(what: &str, r: Result<Value, leon_core::ApiError>) {
    if let Err(e) = r {
        eprintln!("leon: {what} failed: {}", e.message);
    }
}
