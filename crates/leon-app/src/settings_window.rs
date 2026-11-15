//! The Settings window (⌘,): init.lua's settings with a control each. It reads init.lua itself, as
//! `leon config get` does, and every change goes through `leon config set`'s code
//! (`leon_lua::rewrite::write_setting`), then the local server reloads, so init.lua stays the one
//! place settings live. A setting init.lua sets by code that can't be rewritten shows read-only,
//! with `leon config set`'s own message.

use std::path::{Path, PathBuf};
use std::rc::Rc;

use gpui::{
    div, prelude::*, px, rgb, size, App, Bounds, Context, SharedString, Subscription, TitlebarOptions, Window,
    WindowBounds, WindowHandle, WindowOptions,
};
use leon_lua::rewrite::{read_only, write_setting};
use leon_lua::settings::{load_theme, theme_names, themes_dir, FONT_SIZES};
use serde_json::{json, Value};

use crate::look::Look;
use crate::remote::Remote;
use crate::theme::{Theme, SECONDARY_TEXT, UI_TEXT};

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

/// The notification kinds, as the window names them.
const NOTIFICATIONS: [(&str, &str); 3] = [
    ("notifications.done", "A long run finished while leon was in the background"),
    ("notifications.blocked", "A program is waiting for you"),
    ("notifications.error", "A program reported an error"),
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
    _appearance: Subscription,
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
    Ok(Loaded { settings: cfg.settings.to_json(), sources: cfg.setting_sources, src, themes })
}

impl SettingsWindow {
    /// Open the window on `config`, the init.lua the local `server` loaded.
    pub fn open(config: PathBuf, server: Rc<Remote>, cx: &mut App) -> Option<WindowHandle<SettingsWindow>> {
        let bounds = Bounds::centered(None, size(px(720.), px(760.)), cx);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: Some(TitlebarOptions { title: Some("Settings".into()), ..Default::default() }),
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
        let mut sw = SettingsWindow {
            config,
            server,
            loaded: None,
            error: None,
            fonts,
            dark: crate::view::is_dark(window.appearance()),
            _appearance: appearance,
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

    /// Set `key` to `value` in init.lua (what each control does), then reload the server and read
    /// the file again. A read-only setting is left alone.
    pub fn choose(&mut self, key: &str, value: Value, cx: &mut Context<Self>) {
        if self.row(key).is_none_or(|r| r.read_only.is_some()) {
            return;
        }
        let (config, key, server) = (self.config.clone(), key.to_string(), self.server.clone());
        cx.spawn(async move |this, cx| {
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
            let loaded = cx.background_executor().spawn(async move { load(&config) }).await;
            this.update(cx, |sw, cx| {
                sw.error = error.or(loaded.as_ref().err().cloned());
                if let Ok(l) = loaded {
                    sw.loaded = Some(l);
                }
                cx.notify();
            })
        })
        .detach();
    }

    /// The theme this window draws in: the one init.lua names for the appearance now.
    fn theme(&self) -> Theme {
        let settings = self.loaded.as_ref().map_or(Value::Null, |l| l.settings.clone());
        Look::resolve(&settings, &themes_dir(&self.config)).theme(self.dark)
    }
}

impl Render for SettingsWindow {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.theme();
        let mut body = div().flex().flex_col().gap(px(28.)).p(px(28.));
        if self.loaded.is_some() {
            body = body
                .child(self.theme_section(&t, cx))
                .child(self.font_section(&t, cx))
                .child(self.toggles_section(&t, cx));
        }
        let footer = match &self.error {
            Some(e) => div().text_color(rgb(t.error)).child(e.clone()),
            None => div().text_color(rgb(t.muted)).child(format!("Saved to {}", self.config.display())),
        };
        div()
            .id("settings")
            .size_full()
            .overflow_y_scroll()
            .bg(rgb(t.chrome))
            .text_color(rgb(t.text))
            .font_family(".SystemUIFont")
            .text_size(px(UI_TEXT))
            .child(body.child(footer.text_size(px(SECONDARY_TEXT))))
    }
}

impl SettingsWindow {
    fn section(t: &Theme, title: &str) -> gpui::Div {
        div()
            .flex()
            .flex_col()
            .gap(px(10.))
            .child(div().text_size(px(SECONDARY_TEXT)).text_color(rgb(t.muted)).child(title.to_uppercase()))
    }

    /// A setting's read-only line under its control.
    fn locked(t: &Theme, row: &Option<Row>) -> Option<gpui::Div> {
        let why = row.as_ref()?.read_only.clone()?;
        Some(div().text_size(px(SECONDARY_TEXT)).text_color(rgb(t.muted)).child(why))
    }

    fn theme_section(&self, t: &Theme, cx: &mut Context<Self>) -> gpui::Div {
        let row = self.row("theme");
        let chosen = |side: &str| row.as_ref().and_then(|r| r.value[side].as_str()).unwrap_or_default().to_string();
        let (dark, light) = (chosen("dark"), chosen("light"));
        let locked = row.as_ref().is_some_and(|r| r.read_only.is_some());
        let column = |side: &'static str, title: &str, cx: &mut Context<Self>| {
            let name = if side == "dark" { &dark } else { &light };
            let themes = &self.loaded.as_ref().expect("loaded").themes;
            let shown = themes.iter().find(|(n, _)| n == name).and_then(|(_, p)| *p);
            let mut list = div().flex().flex_col().gap(px(2.));
            for (n, p) in themes {
                let on = n == name;
                let pair = if side == "dark" { (n.clone(), light.clone()) } else { (dark.clone(), n.clone()) };
                list = list.child(
                    div()
                        .id(SharedString::from(format!("theme-{side}-{n}")))
                        .flex()
                        .items_center()
                        .gap(px(10.))
                        .px(px(10.))
                        .py(px(6.))
                        .rounded(px(6.))
                        .when(on, |d| d.bg(rgb(t.selected)).text_color(rgb(t.title)))
                        .when(!on && !locked && p.is_some(), |d| {
                            d.cursor_pointer().hover(|d| d.bg(rgb(t.hover))).on_click(cx.listener(
                                move |sw, _, _, cx| sw.choose("theme", json!({ "dark": pair.0, "light": pair.1 }), cx),
                            ))
                        })
                        .child(dots(p.as_ref()))
                        .child(div().flex_1().child(n.clone()))
                        .when(p.is_none(), |d| d.child(div().text_color(rgb(t.error)).child("won't load"))),
                );
            }
            div()
                .flex_1()
                .flex()
                .flex_col()
                .gap(px(10.))
                .child(div().text_color(rgb(t.title)).child(title.to_string()))
                .child(preview(shown.as_ref()))
                .child(list)
        };
        Self::section(t, "Theme")
            .child(div().flex().gap(px(20.)).child(column("dark", "When macOS is dark", cx)).child(column(
                "light",
                "When macOS is light",
                cx,
            )))
            .children(Self::locked(t, &row))
    }

    fn font_section(&self, t: &Theme, cx: &mut Context<Self>) -> gpui::Div {
        let family_row = self.row("font.family");
        let size_row = self.row("font.size");
        let family = family_row.as_ref().and_then(|r| r.value.as_str()).unwrap_or("Menlo").to_string();
        let pt = size_row.as_ref().and_then(|r| r.value.as_f64()).unwrap_or(13.);
        let mut fonts = self.fonts.clone();
        if !fonts.contains(&family) {
            fonts.insert(0, family.clone());
        }
        let at = fonts.iter().position(|f| *f == family).unwrap_or(0);
        let step = |by: isize| fonts[(at as isize + by).rem_euclid(fonts.len() as isize) as usize].clone();
        let family_locked = family_row.as_ref().is_some_and(|r| r.read_only.is_some()) || fonts.len() < 2;
        let size_locked = size_row.as_ref().is_some_and(|r| r.read_only.is_some());
        let (back, next) = (step(-1), step(1));
        let smaller = (pt - 1.).max(FONT_SIZES.0);
        let bigger = (pt + 1.).min(FONT_SIZES.1);
        Self::section(t, "Font")
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(24.))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .child(button(t, "font-back", "‹", !family_locked, cx, move |sw, cx| {
                                sw.choose("font.family", json!(back), cx)
                            }))
                            .child(
                                div().min_w(px(150.)).flex().justify_center().font_family(family.clone()).child(family),
                            )
                            .child(button(t, "font-next", "›", !family_locked, cx, move |sw, cx| {
                                sw.choose("font.family", json!(next), cx)
                            })),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .child(button(
                                t,
                                "size-down",
                                "−",
                                !size_locked && pt > FONT_SIZES.0,
                                cx,
                                move |sw, cx| sw.choose("font.size", json!(smaller), cx),
                            ))
                            .child(div().min_w(px(56.)).flex().justify_center().child(format!("{pt} pt")))
                            .child(button(t, "size-up", "+", !size_locked && pt < FONT_SIZES.1, cx, move |sw, cx| {
                                sw.choose("font.size", json!(bigger), cx)
                            })),
                    ),
            )
            .children(Self::locked(t, &family_row))
            .children(Self::locked(t, &size_row))
    }

    fn toggles_section(&self, t: &Theme, cx: &mut Context<Self>) -> gpui::Div {
        let mut notes = Self::section(t, "Notify me when");
        for (key, label) in NOTIFICATIONS {
            notes = notes.child(self.toggle(t, key, label, cx));
        }
        div().flex().flex_col().gap(px(28.)).child(notes).child(Self::section(t, "Selection").child(self.toggle(
            t,
            "copy_on_select",
            "Copy text as I select it",
            cx,
        )))
    }

    /// A switch for a true/false setting, with its label.
    fn toggle(&self, t: &Theme, key: &'static str, label: &str, cx: &mut Context<Self>) -> gpui::Div {
        let row = self.row(key);
        let on = row.as_ref().is_some_and(|r| r.value == true);
        let locked = row.as_ref().is_none_or(|r| r.read_only.is_some());
        div()
            .flex()
            .flex_col()
            .gap(px(4.))
            .child(
                div()
                    .id(SharedString::from(format!("toggle-{key}")))
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(px(16.))
                    .child(div().text_color(rgb(if locked { t.muted } else { t.text })).child(label.to_string()))
                    .child(switch(t, on, locked))
                    .when(!locked, |d| {
                        d.cursor_pointer().on_click(cx.listener(move |sw, _, _, cx| sw.choose(key, json!(!on), cx)))
                    }),
            )
            .children(Self::locked(t, &row))
    }
}

/// A small square button with a label; inert when not `enabled`.
fn button(
    t: &Theme,
    id: &'static str,
    label: &'static str,
    enabled: bool,
    cx: &mut Context<SettingsWindow>,
    run: impl Fn(&mut SettingsWindow, &mut Context<SettingsWindow>) + 'static,
) -> impl IntoElement {
    div()
        .id(id)
        .size(px(26.))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(6.))
        .border_1()
        .border_color(rgb(t.border))
        .text_color(rgb(if enabled { t.title } else { t.muted }))
        .when(enabled, |d| {
            d.cursor_pointer().hover(|d| d.bg(rgb(t.hover))).on_click(cx.listener(move |sw, _, _, cx| run(sw, cx)))
        })
        .child(label)
}

/// An on/off switch.
fn switch(t: &Theme, on: bool, locked: bool) -> impl IntoElement {
    let track = if on { t.accent } else { t.border };
    div()
        .w(px(34.))
        .h(px(20.))
        .flex_none()
        .rounded_full()
        .p(px(2.))
        .flex()
        .when(on, |d| d.justify_end())
        .bg(rgb(track))
        .when(locked, |d| d.opacity(0.5))
        .child(div().size(px(16.)).rounded_full().bg(rgb(t.title)))
}

/// A theme's ANSI 1–6 as dots, for its row in the list.
fn dots(p: Option<&Theme>) -> impl IntoElement {
    let mut row = div().flex().gap(px(2.)).p(px(3.)).rounded(px(4.));
    if let Some(p) = p {
        row = row.bg(rgb(p.bg));
        for c in &p.ansi[1..7] {
            row = row.child(div().size(px(7.)).rounded_full().bg(rgb(*c)));
        }
    }
    row.min_w(px(57.))
}

/// A small picture of a theme: its chrome beside a pane with a prompt, a command's coloured output
/// and the cursor.
fn preview(p: Option<&Theme>) -> impl IntoElement {
    let Some(p) = p else { return div().h(px(132.)) };
    let line = |parts: Vec<(&'static str, u32)>| {
        parts.into_iter().fold(div().flex(), |d, (text, c)| d.child(div().text_color(rgb(c)).child(text)))
    };
    div()
        .h(px(132.))
        .flex()
        .rounded(px(8.))
        .overflow_hidden()
        .border_1()
        .border_color(rgb(p.border))
        .font_family("Menlo")
        .text_size(px(11.))
        .child(
            div()
                .w(px(56.))
                .bg(rgb(p.chrome))
                .flex()
                .flex_col()
                .gap(px(6.))
                .p(px(8.))
                .child(div().h(px(6.)).w(px(32.)).rounded(px(3.)).bg(rgb(p.muted)))
                .child(div().h(px(6.)).w(px(24.)).rounded(px(3.)).bg(rgb(p.selected)))
                .child(div().flex().gap(px(4.)).child(div().size(px(6.)).rounded_full().bg(rgb(p.working)))),
        )
        .child(
            div()
                .flex_1()
                .bg(rgb(p.bg))
                .p(px(10.))
                .flex()
                .flex_col()
                .gap(px(3.))
                .child(line(vec![("~/src ", p.ansi[4]), ("% ", p.fg), ("cargo test", p.fg)]))
                .child(line(vec![("   Compiling ", p.ansi[2]), ("leon", p.fg)]))
                .child(line(vec![("warning", p.ansi[3]), (": unused import", p.fg)]))
                .child(line(vec![("error", p.ansi[1]), (": 1 test failed", p.fg)]))
                .child(
                    line(vec![("~/src ", p.ansi[4]), ("% ", p.fg)]).child(div().w(px(7.)).h(px(13.)).bg(rgb(p.cursor))),
                ),
        )
}
