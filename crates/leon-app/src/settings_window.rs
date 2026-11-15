//! The Settings window (⌘,): init.lua's settings with a control each. It reads init.lua itself, as
//! `leon config get` does, and every change goes through `leon config set`'s code
//! (`leon_lua::rewrite::write_setting`), then the local server reloads, so init.lua stays the one
//! place settings live. A setting init.lua sets by code that can't be rewritten shows read-only,
//! with `leon config set`'s own message.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use gpui::{
    div, point, prelude::*, px, rgb, size, App, Bounds, Context, FocusHandle, FontWeight, KeyDownEvent, SharedString,
    Subscription, TitlebarOptions, Window, WindowBounds, WindowHandle, WindowOptions,
};
use leon_lua::rewrite::{read_only, write_setting};
use leon_lua::settings::{load_theme, theme_names, themes_dir, FONT_SIZES};
use serde_json::{json, Value};

use crate::icons::{Icon, ROW_ICON};
use crate::look::Look;
use crate::remote::Remote;
use crate::theme::{Theme, SECONDARY_TEXT, UI_TEXT};

#[path = "settings_window_rows.rs"]
mod rows;
#[path = "settings_window_theme.rs"]
mod theme_picker;

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
    /// The last write's refusal, or why init.lua doesn't load.
    error: Option<String>,
    /// The installed families of `FONTS`.
    fonts: Vec<String>,
    dark: bool,
    /// Changes not written yet, in the order they were made; `saving` while they are written.
    queue: VecDeque<(String, Value)>,
    saving: bool,
    focus: FocusHandle,
    _appearance: Subscription,
    _activation: Subscription,
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
        let bounds = Bounds::centered(None, size(px(720.), px(780.)), cx);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            // drawn in the theme's chrome, as the main window's is, not in the macOS appearance
            titlebar: Some(TitlebarOptions {
                title: Some("Settings".into()),
                appears_transparent: true,
                traffic_light_position: Some(point(px(14.), px(13.))),
            }),
            window_min_size: Some(size(px(560.), px(420.))),
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
            cx.notify();
        });
        // init.lua may have been edited by hand while the window was in the background
        let activation = cx.observe_window_activation(window, |sw, window, cx| {
            if window.is_window_active() && !sw.saving {
                sw.reload(cx);
            }
        });
        let focus = cx.focus_handle();
        window.focus(&focus);
        let mut sw = SettingsWindow {
            config,
            server,
            loaded: None,
            error: None,
            fonts,
            dark: crate::view::is_dark(window.appearance()),
            queue: VecDeque::new(),
            saving: false,
            focus,
            _appearance: appearance,
            _activation: activation,
        };
        sw.reload(cx);
        sw
    }

    /// Read init.lua again.
    fn reload(&mut self, cx: &mut Context<Self>) {
        let config = self.config.clone();
        cx.spawn(async move |this, cx| {
            let loaded = cx.background_executor().spawn(async move { load(&config) }).await;
            this.update(cx, |sw, cx| {
                match loaded {
                    Ok(l) => sw.loaded = Some(l),
                    Err(e) => sw.error = Some(e),
                }
                cx.notify();
            })
        })
        .detach();
    }

    /// `key` as init.lua leaves it, and whether it can be changed here (None before the first read).
    pub fn row(&self, key: &str) -> Option<Row> {
        let l = self.loaded.as_ref()?;
        let value = key.split('.').try_fold(l.settings.clone(), |v, part| v.get(part).cloned())?;
        Some(Row { value, read_only: read_only(&self.config, &l.src, &l.sources, key) })
    }

    /// The last change's refusal, or why init.lua doesn't load.
    pub fn error(&self) -> Option<String> {
        self.error.clone()
    }

    /// Changes are still being written.
    pub fn saving(&self) -> bool {
        self.saving
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
        self.queue.push_back((key.to_string(), value));
        cx.notify();
        if !self.saving {
            self.saving = true;
            self.error = None;
            self.write_queue(cx);
        }
    }

    /// Write the queued changes, reloading the server after each, then read init.lua again.
    fn write_queue(&mut self, cx: &mut Context<Self>) {
        let (config, server) = (self.config.clone(), self.server.clone());
        cx.spawn(async move |this, cx| {
            while let Some((key, value)) = this.update(cx, |sw, _| sw.queue.pop_front()).ok().flatten() {
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
            let loaded = cx.background_executor().spawn(async move { load(&config) }).await;
            this.update(cx, |sw, cx| {
                sw.saving = false;
                match loaded {
                    Ok(l) => sw.loaded = Some(l),
                    Err(e) => sw.error = Some(e),
                }
                cx.notify();
            })
        })
        .detach();
    }

    /// The theme this window draws in: the one init.lua names for the appearance now.
    fn theme(&self) -> Theme {
        self.loaded.as_ref().map_or_else(Look::default, |l| l.look.clone()).theme(self.dark)
    }
}

impl Render for SettingsWindow {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.theme();
        let mut body = div().flex().flex_col().gap(px(32.)).px(px(28.)).pb(px(28.));
        if self.loaded.is_some() {
            body = body
                .child(self.theme_section(&t, cx))
                .child(self.font_section(&t, cx))
                .child(self.toggles_section(&t, cx));
        }
        let footer = match (&self.error, self.saving) {
            (_, true) => div().text_color(rgb(t.muted)).child("Saving…"),
            (Some(e), _) => div().text_color(rgb(t.error)).child(e.clone()),
            (None, _) => {
                div().text_color(rgb(t.muted)).child(format!("Every change is written to {}", shown(&self.config)))
            }
        };
        div()
            .id("settings")
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|_, e: &KeyDownEvent, window, _| {
                let k = &e.keystroke;
                if k.key == "escape" || (k.modifiers.platform && k.key == "w") {
                    window.remove_window();
                }
            }))
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(t.chrome))
            .text_color(rgb(t.text))
            .font_family(".SystemUIFont")
            .text_size(px(UI_TEXT))
            .child(
                div()
                    .h(px(38.))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_color(rgb(t.title))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("Settings"),
            )
            .child(
                div()
                    .id("settings-body")
                    .flex_1()
                    .overflow_y_scroll()
                    .child(body.child(footer.text_size(px(SECONDARY_TEXT)))),
            )
    }
}

/// `path` with the home folder as `~`.
fn shown(path: &Path) -> String {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    match home.as_deref().and_then(|h| path.strip_prefix(h).ok()) {
        Some(rest) => format!("~/{}", rest.display()),
        None => path.display().to_string(),
    }
}
