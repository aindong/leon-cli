//! The Settings window (⌘,): a sidebar of sections and a search field over init.lua's settings,
//! each with a control, the keymap and the agents' skill. It reads init.lua itself, as
//! `leon config get` does, and every change goes through `leon config set`'s code
//! (`leon_lua::rewrite::write_setting`), then the local server reloads, so init.lua stays the one
//! place settings live. A setting init.lua sets by code that can't be rewritten shows read-only,
//! with `leon config set`'s own message.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use gpui::{
    div, point, prelude::*, px, rgb, size, AnyWindowHandle, App, Bounds, Context, FocusHandle, FontWeight, Hsla,
    KeyDownEvent, SharedString, Subscription, TitlebarOptions, Window, WindowBackgroundAppearance as Bg, WindowBounds,
    WindowHandle, WindowOptions,
};
use leon_lua::rewrite::{read_only, write_setting};
use leon_lua::settings::{load_theme, theme_names, themes_dir, FONT_SIZES};
use serde_json::{json, Value};

use crate::glass::Glass;
use crate::icons::{Icon, ROW_ICON};
use crate::look::Look;
use crate::remote::Remote;
use crate::theme::{Theme, SECONDARY_TEXT, UI_TEXT};

#[path = "settings_window_agents.rs"]
mod agents;
#[path = "settings_window_glass.rs"]
mod glass_ui;
#[path = "settings_window_hosts.rs"]
mod hosts;
#[path = "settings_window_lists.rs"]
mod lists;
#[path = "settings_window_patterns.rs"]
mod patterns;
#[path = "settings_window_rows.rs"]
mod rows;
#[path = "settings_window_sections.rs"]
mod sections;
#[path = "settings_window_server.rs"]
mod server;
#[path = "settings_window_shader.rs"]
mod shader;
#[path = "settings_window_tailscale.rs"]
mod tailscale;
#[path = "settings_window_theme.rs"]
mod theme_picker;
#[path = "settings_window_web.rs"]
pub(crate) mod web;

pub use crate::agent_skill::{parse_agents, AgentRow};
pub use lists::ShortcutRow;
pub use sections::Section;
pub use server::{running_for, stop_warning, ServerInfo};
pub use web::{web_state, WebState};

/// Monospaced families the font control steps through, when installed (init.lua takes any).
// shortcut: a fixed list, not every installed monospace font; upgrade with a text field.
const FONTS: [&str; 16] = [
    "Menlo",
    "Monaco",
    "SF Mono",
    "JetBrains Mono",
    "Fira Code",
    "Source Code Pro",
    "IBM Plex Mono",
    "Hack",
    "Cascadia Code",
    "Iosevka",
    "Roboto Mono",
    "Ubuntu Mono",
    "Inconsolata",
    "Victor Mono",
    "Geist Mono",
    "Courier New",
];

/// One setting as the window shows it.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub value: Value,
    /// Why it can't be changed here (`leon config set`'s message), if it can't.
    pub read_only: Option<String>,
}

/// init.lua as last read.
struct Loaded {
    settings: Value,
    sources: std::collections::BTreeMap<String, String>,
    src: String,
    /// Every theme a setting can name, with its colours (None: its file doesn't load).
    themes: Vec<(String, Option<Theme>)>,
    /// The themes this window draws in, resolved once per read.
    look: Look,
}

pub struct SettingsWindow {
    config: PathBuf,
    server: Rc<Remote>,
    loaded: Option<Loaded>,
    /// The last write's refusal.
    error: Option<String>,
    /// Why init.lua didn't load, the last time it was read.
    load_error: Option<String>,
    /// The installed families of `FONTS`.
    fonts: Vec<String>,
    dark: bool,
    /// Changes not written yet, in the order they were made; `saving` while they are written.
    /// Shared with the writer, so a change made just before the window closes is still written.
    queue: Rc<RefCell<VecDeque<(String, Value)>>>,
    saving: bool,
    /// The section shown when the search field is empty.
    section: Section,
    query: String,
    /// The hint pattern field: what is typed, the pattern it replaces (None: a new one), and why
    /// the last Add was refused.
    pattern: patterns::TextField,
    /// Tailscale's port and tag fields, and the web page's certificate and key files.
    port: patterns::TextField,
    tag: patterns::TextField,
    web_cert: patterns::TextField,
    web_key: patterns::TextField,
    /// The keymap's bindings, once the server has sent them.
    shortcuts: Vec<ShortcutRow>,
    /// `leon skill status`, once read (Err: why it couldn't be).
    agents: Option<Result<Vec<AgentRow>, String>>,
    agents_ui: agents::AgentsUi,
    /// This Mac's server, once asked (Err: why it didn't answer).
    server_info: Option<Result<server::ServerInfo, String>>,
    /// The main window's hosts, followed while it is open.
    hosts: Vec<crate::hosts::HostLine>,
    rename: hosts::RenameField,
    /// Restart stopped the server and waits for the main window to see it gone.
    restarting: bool,
    _workspace: Option<Subscription>,
    glass: Glass,
    /// What GPUI was asked for, oldest first (GPUI's test window keeps no background).
    backgrounds: Vec<Bg>,
    /// Reduce Transparency as a test set it; None asks the system.
    reduce_transparency: Option<bool>,
    reduced: bool,
    handle: AnyWindowHandle,
    renders: usize,
    /// `server.status`'s `tailnet`, fetched every 2 s (settings_window_tailscale.rs).
    tailnet: Option<Value>,
    open: web::Open,
    focus: FocusHandle,
    _appearance: Subscription,
    _activation: Subscription,
    _bounds: Subscription,
}

/// Read the config at `config` and the themes beside it (off the UI thread: Lua runs).
fn load(config: &Path) -> Result<Loaded, String> {
    let (cfg, src) = match std::fs::read_to_string(config) {
        Ok(src) => (leon_lua::load(config).map_err(|e| e.to_string())?, src),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => (leon_lua::Config::default(), String::new()),
        Err(e) => return Err(format!("can't read {}: {e}", config.display())),
    };
    let dir = themes_dir(config);
    let themes = theme_names(&dir).into_iter().map(|n| (n.clone(), load_theme(&dir, &n).ok())).collect();
    let settings = cfg.settings.to_json();
    let look = Look::resolve(&settings, &dir);
    Ok(Loaded { settings, sources: cfg.setting_sources, src, themes, look })
}

impl SettingsWindow {
    /// Open the window on `config`, the init.lua the local `server` loaded.
    pub fn open(config: PathBuf, server: Rc<Remote>, cx: &mut App) -> Option<WindowHandle<SettingsWindow>> {
        let bounds = Bounds::centered(None, size(px(860.), px(780.)), cx);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            // drawn in the theme's chrome, as the main window's is, not in the macOS appearance
            titlebar: Some(TitlebarOptions {
                title: Some("Settings".into()),
                appears_transparent: true,
                traffic_light_position: Some(point(px(14.), px(13.))),
            }),
            window_min_size: Some(size(px(640.), px(420.))),
            ..WindowOptions::default()
        };
        let opened = cx.open_window(options, |window, cx| cx.new(|cx| SettingsWindow::new(config, server, window, cx)));
        opened.map_err(|e| eprintln!("leon: can't open Settings: {e}")).ok()
    }

    fn new(config: PathBuf, server: Rc<Remote>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let installed = window.text_system().all_font_names();
        let fonts = FONTS.iter().filter(|f| installed.iter().any(|i| i == *f)).map(|f| f.to_string()).collect();
        let appearance = cx.observe_window_appearance(window, |sw, window, cx| {
            sw.dark = crate::view::is_dark(window.appearance());
            sw.update_glass(window, cx);
            cx.notify();
        });
        // init.lua may have been edited by hand while the window was in the background, and
        // Reduce Transparency changed in System Settings
        let activation = cx.observe_window_activation(window, |sw, window, cx| {
            if !window.is_window_active() {
                return;
            }
            // changed in System Settings meanwhile; a write under way doesn't wait for it
            sw.note_reduce_transparency();
            sw.update_glass(window, cx);
            if !sw.saving {
                sw.reload(cx);
                sw.read_shortcuts(cx);
                sw.read_server(cx);
                if sw._workspace.is_none() {
                    sw.follow_workspace(cx); // leon's window opened since
                }
            }
        });
        // full screen has nothing behind it to show
        let bounds = cx.observe_window_bounds(window, |sw, window, cx| sw.update_glass(window, cx));
        let focus = cx.focus_handle();
        window.focus(&focus);
        let mut sw = SettingsWindow {
            config,
            server,
            loaded: None,
            error: None,
            load_error: None,
            fonts,
            dark: crate::view::is_dark(window.appearance()),
            queue: Rc::default(),
            saving: false,
            section: Section::General,
            query: String::new(),
            pattern: patterns::TextField::new(cx),
            port: patterns::TextField::new(cx),
            tag: patterns::TextField::new(cx),
            web_cert: patterns::TextField::new(cx),
            web_key: patterns::TextField::new(cx),
            shortcuts: Vec::new(),
            agents: None,
            agents_ui: Default::default(),
            server_info: None,
            hosts: Vec::new(),
            rename: hosts::RenameField::new(cx),
            restarting: false,
            _workspace: None,
            glass: Glass::OPAQUE,
            backgrounds: Vec::new(),
            reduce_transparency: None,
            reduced: false,
            handle: window.window_handle(),
            renders: 0,
            tailnet: None,
            open: web::Open::default(),
            focus,
            _appearance: appearance,
            _activation: activation,
            _bounds: bounds,
        };
        sw.note_reduce_transparency();
        sw.reload(cx);
        sw.read_shortcuts(cx);
        sw.read_server(cx);
        // once ⌘,'s update of the main window is over: it can't be read while it opens this one
        cx.spawn(async move |this, cx| this.update(cx, |sw, cx| sw.follow_workspace(cx))).detach();
        // the app runs as `leon app`: its own binary says what `leon skill status` says
        match std::env::current_exe().ok().filter(|e| e.file_name().is_some_and(|n| n == "leon")) {
            Some(leon) => sw.read_agents(leon, cx),
            None => sw.agents = Some(Err("This isn't the leon binary: run leon skill status in a terminal".into())),
        }
        sw.watch_tailnet(cx);
        sw
    }

    /// Read init.lua again.
    fn reload(&mut self, cx: &mut Context<Self>) {
        let config = self.config.clone();
        cx.spawn(async move |this, cx| {
            let loaded = cx.background_executor().spawn(async move { load(&config) }).await;
            this.update(cx, |sw, cx| sw.loaded(loaded, cx))
        })
        .detach();
    }

    fn loaded(&mut self, loaded: Result<Loaded, String>, cx: &mut Context<Self>) {
        match loaded {
            Ok(l) => (self.loaded, self.load_error) = (Some(l), None),
            Err(e) => self.load_error = Some(e),
        }
        self.glass_later(cx);
        cx.notify();
    }

    /// `key` as init.lua leaves it, and whether it can be changed here (None before the first read).
    pub fn row(&self, key: &str) -> Option<Row> {
        let l = self.loaded.as_ref()?;
        let value = key.split('.').try_fold(l.settings.clone(), |v, part| v.get(part).cloned())?;
        Some(Row { value, read_only: read_only(&self.config, &l.src, &l.sources, key) })
    }

    /// The last change's refusal, or why init.lua doesn't load.
    pub fn error(&self) -> Option<String> {
        self.error.clone().or_else(|| self.load_error.clone())
    }

    /// Changes are still being written.
    pub fn saving(&self) -> bool {
        self.saving
    }

    /// How many times the window has rendered.
    pub fn renders(&self) -> usize {
        self.renders
    }

    /// Set `key` to `value` in init.lua (what each control does), then reload the server and read
    /// the file again. The window shows the value at once, so a second click builds on it, and
    /// changes are written one at a time, in order. A read-only setting is left alone.
    pub fn choose(&mut self, key: &str, value: Value, cx: &mut Context<Self>) {
        if self.row(key).is_none_or(|r| r.read_only.is_some()) {
            return;
        }
        if let Some(l) = self.loaded.as_mut() {
            let parts: Vec<&str> = key.split('.').collect();
            let (last, path) = parts.split_last().expect("a key");
            if let Some(Value::Object(o)) = path.iter().try_fold(&mut l.settings, |v, p| v.get_mut(*p)) {
                o.insert(last.to_string(), value.clone());
            }
        }
        self.queue.borrow_mut().push_back((key.to_string(), value));
        cx.notify();
        if !self.saving {
            self.saving = true;
            self.error = None;
            self.write_queue(cx);
        }
    }

    /// Write the queued changes, reloading the server after each, then read init.lua again.
    fn write_queue(&mut self, cx: &mut Context<Self>) {
        let (config, server, queue) = (self.config.clone(), self.server.clone(), self.queue.clone());
        cx.spawn(async move |this, cx| loop {
            // the queue is drained even if the window closes: a change shown was asked for
            loop {
                // taken out first: a borrow kept across the awaits below would meet a click's push
                let next = queue.borrow_mut().pop_front();
                let Some((key, value)) = next else { break };
                let path = config.clone();
                let written = cx.background_executor().spawn(async move { write_setting(&path, &key, &value) }).await;
                let mut error = written.err();
                if error.is_none() {
                    // as `leon config set` does: the running server follows the file
                    let (tx, rx) = futures::channel::oneshot::channel();
                    server.spawn("server.config_reload", json!({}), move |r| drop(tx.send(r)));
                    if let Ok(Err(e)) = rx.await {
                        error = Some(format!("init.lua is written, but the server didn't reload it: {}", e.message));
                    }
                }
                if error.is_some() {
                    this.update(cx, |sw, _| sw.error = error).ok();
                }
            }
            let path = config.clone();
            let loaded = cx.background_executor().spawn(async move { load(&path) }).await;
            if !queue.borrow().is_empty() {
                continue; // a change made while the file was read: write it, and read again after
            }
            let _ = this.update(cx, |sw, cx| {
                sw.saving = false;
                sw.loaded(loaded, cx);
            });
            break;
        })
        .detach();
    }

    /// The theme this window draws in: the one init.lua names for the appearance now.
    pub fn theme(&self) -> Theme {
        self.loaded.as_ref().map_or_else(Look::default, |l| l.look.clone()).theme(self.dark)
    }

    /// A key anywhere but a text field (a hint pattern, Tailscale's port or tag): the search
    /// field's, ↑/↓ for the sidebar, Escape and ⌘W to close.
    fn key_down(&mut self, e: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let k = &e.keystroke;
        let m = &k.modifiers;
        if m.platform {
            if k.key == "w" {
                window.remove_window();
            }
            return;
        }
        match k.key.as_str() {
            "escape" if self.query.is_empty() => window.remove_window(),
            "escape" => self.query.clear(),
            "up" | "down" => {
                let all = Section::ALL;
                let at = all.iter().position(|s| *s == self.section).unwrap_or(0);
                let to = if k.key == "up" { at.saturating_sub(1) } else { (at + 1).min(all.len() - 1) };
                self.section = all[to];
                self.query.clear();
            }
            "backspace" => {
                self.query.pop();
            }
            "space" => self.query.push(' '),
            _ => match k.key_char.as_deref().filter(|c| !c.is_empty() && !m.control && !m.alt) {
                Some(typed) => self.query.extend(typed.chars().filter(|c| !c.is_control())),
                None => return,
            },
        }
        cx.notify();
    }
}

impl Render for SettingsWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.renders += 1;
        for f in [&mut self.pattern, &mut self.port, &mut self.tag] {
            f.typing = f.focused(window);
        }
        let t = self.theme();
        let (text, color) = match (self.error(), self.saving) {
            (_, true) => ("Saving…".to_string(), t.muted),
            // a refusal can name a file in the home folder: the window never shows the login
            (Some(e), _) => (hide_home(&e, std::env::var_os("HOME").map(PathBuf::from).as_deref()), t.error),
            (None, _) => {
                let home = std::env::var_os("HOME").map(PathBuf::from);
                (format!("Every change is written to {}", home_relative(&self.config, home.as_deref())), t.muted)
            }
        };
        let full: SharedString = self.config.display().to_string().into();
        let footer = div()
            .id("settings-footer")
            .text_size(px(SECONDARY_TEXT))
            .text_color(rgb(color))
            .child(text)
            .tooltip(move |_, cx| cx.new(|_| crate::view::Tip(full.clone(), t)).into());
        div()
            .id("settings")
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::key_down))
            .size_full()
            .flex()
            .bg(self.fill(&t))
            .text_color(rgb(t.text))
            .font_family(".SystemUIFont")
            .text_size(px(UI_TEXT))
            .child(self.sidebar(&t, cx))
            .child(
                div().flex_1().min_w(px(0.)).flex().flex_col().child(div().h(px(38.)).flex_none()).child(
                    div()
                        .id("settings-body")
                        .flex_1()
                        .overflow_y_scroll()
                        .child(self.page(&t, cx).px(px(28.)).pb(px(28.)).child(footer)),
                ),
            )
    }
}

/// `text` with each mention of the `home` folder as `~`.
pub fn hide_home(text: &str, home: Option<&Path>) -> String {
    match home.map(|h| h.display().to_string()).filter(|h| h.len() > 1) {
        Some(h) => text.replace(&h, "~"),
        None => text.to_string(),
    }
}

/// `path` with the `home` folder as `~`, so the window (and every screenshot of it) doesn't show
/// the login name; the full path is the footer's tooltip.
pub fn home_relative(path: &Path, home: Option<&Path>) -> String {
    match home.and_then(|h| path.strip_prefix(h).ok()) {
        Some(rest) if rest.as_os_str().is_empty() => "~".into(),
        Some(rest) => format!("~/{}", rest.display()),
        None => path.display().to_string(),
    }
}
