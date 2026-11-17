//! Themes in the workspace: the palette's theme list (Change Theme…, `client.theme.change`), which
//! draws the window in the selected theme until ↩ keeps it or the list closes, and the cross-fade
//! from one theme to the next. The fade asks GPUI for animation frames from `render` while it
//! moves (ADR R6, as the overview's motion does) and is over in a slide's time.

use leon_lua::rewrite::write_setting;
use leon_lua::settings::{load_theme, theme_names, themes_dir};

use super::*;
use crate::palette::Item;

/// How far a pane's colours dip toward the window at the middle of a change: the server sends the
/// panes' text again in the new colours, so the old text fades out before the new comes in.
const DIP: f32 = 0.8;

/// The theme the window is drawn in, on its way to the one wanted.
pub(super) struct ThemeUi {
    /// The theme last wanted, which the colours are heading for.
    to: Theme,
    /// The colours on screen when the change to `to` started, and when (None: at rest).
    from: Theme,
    at: Option<Instant>,
    /// How far the panes are dipped this frame, and how far they were when this change started
    /// (one that starts mid-way through another carries on from there, never flashing back).
    dip: f32,
    held: f32,
    /// The setting's theme names (dark, light), as the last settings reply gave them.
    names: (String, String),
    /// The settings were applied once: a change from then on is the user's, and fades.
    settled: bool,
}

impl ThemeUi {
    pub(super) fn new(t: Theme) -> Self {
        let d = leon_lua::settings::Settings::default().theme;
        ThemeUi { to: t, from: t, at: None, dip: 0., held: 0., names: (d.dark, d.light), settled: false }
    }
}

impl Workspace {
    /// The theme the window is drawn in once any change has faded in: the one the theme list
    /// shows, else the settings' for macOS's appearance.
    pub fn theme(&self) -> Theme {
        self.previewed().unwrap_or_else(|| self.look.theme(self.dark))
    }

    /// The colours this frame draws with, part of the way from one theme to the next while a
    /// change fades in.
    pub fn drawn_theme(&self) -> Theme {
        self.theme
    }

    /// A theme change is fading in: render asks for the next animation frame.
    pub fn theme_shifting(&self) -> bool {
        self.themes.at.is_some()
    }

    /// How far a theme change dips the panes' colours this frame (0 at rest).
    pub(super) fn theme_dip(&self) -> f32 {
        self.themes.dip
    }

    /// The selected row of the open palette, if it has any rows.
    pub fn palette_selected_row(&self) -> Option<usize> {
        let n = self.palette_matches().len();
        self.palette.as_ref().filter(|_| n > 0).map(|p| p.selected.min(n - 1))
    }

    /// Head for the theme wanted now, and draw this frame's step of the way: a new one fades in
    /// from the colours on screen, at once with Reduce Motion or before the first settings came
    /// (a launch draws the configured theme straight away). Runs before every frame (`prepare`).
    pub(super) fn follow_theme(&mut self) {
        let want = self.theme();
        let now = self.fade_now();
        if want != self.themes.to {
            let moving = self.themes.settled && !self.reduce_motion();
            (self.themes.from, self.themes.to, self.themes.held) = (self.theme, want, self.themes.dip);
            self.themes.at = moving.then_some(now);
            self.term.panes.values_mut().for_each(|p| p.cache.clear()); // scrollback in the old colours
            self.send_palette();
        }
        let s = &mut self.themes;
        let elapsed = s.at.map(|at| now.saturating_duration_since(at));
        match elapsed.and_then(|e| floats::slide(e, false).map(|p| (p, e.as_secs_f32() / floats::SLIDE.as_secs_f32())))
        {
            Some((p, linear)) => {
                self.theme = s.from.mix(&s.to, p);
                s.dip = (DIP * (1. - (2. * linear - 1.).abs())).max(s.held * (1. - linear));
            }
            None => {
                (self.theme, s.at, s.dip, s.held) = (s.to, None, 0., 0.);
            }
        }
    }

    /// The theme names of a `server.settings` reply; from the first one on, a change fades.
    pub(super) fn note_theme_names(&mut self, settings: &Value) {
        let name = |side: &str, was: &str| settings["theme"][side].as_str().unwrap_or(was).to_string();
        let n = &self.themes.names;
        self.themes.names = (name("dark", &n.0), name("light", &n.1));
        self.themes.settled = true;
    }

    /// The theme the open theme list's selected row names.
    fn previewed(&self) -> Option<Theme> {
        let list = self.palette.as_ref()?.themes.as_ref()?;
        let matches = self.palette_matches();
        let item = matches.get(self.palette.as_ref()?.selected.min(matches.len().checked_sub(1)?))?;
        let name = item.args.as_ref()?["theme"].as_str()?.to_string();
        list.iter().find(|(n, _)| *n == name).map(|(_, t)| *t)
    }

    fn themes_dir(&self) -> PathBuf {
        self.config_path().map(|p| themes_dir(&p)).unwrap_or_default()
    }

    /// `client.theme.change`: keep `{"theme": NAME}`, or open the theme list.
    pub(super) fn theme_action(&mut self, args: Option<&Value>, cx: &mut Context<Self>) {
        match args.and_then(|a| a["theme"].as_str()) {
            Some(name) => self.keep_theme(name.to_string(), cx),
            None => self.open_themes(None, cx),
        }
    }

    /// The theme list: every theme that loads, the built-in ones first, the one in use selected;
    /// `note` says why the last pick wasn't kept.
    fn open_themes(&mut self, note: Option<String>, cx: &mut Context<Self>) {
        let dir = self.themes_dir();
        // shortcut: read on the UI thread, as `apply_settings` resolves the themes; a few small files
        let list: Vec<(String, Theme)> =
            theme_names(&dir).into_iter().filter_map(|n| load_theme(&dir, &n).ok().map(|t| (n, t))).collect();
        let current = if self.dark { &self.themes.names.0 } else { &self.themes.names.1 };
        // a configured theme that no longer loads is drawn as the built-in one: select that
        let shown = self.look.theme(self.dark);
        let selected =
            list.iter().position(|(n, _)| n == current).or_else(|| list.iter().position(|(_, t)| *t == shown));
        let selected = selected.unwrap_or(0);
        let themes = Some(list);
        self.palette = Some(Palette { query: String::new(), selected, ask: None, switch: false, note, themes });
        cx.notify();
    }

    /// The theme list's rows for `query`: each theme by name, with whether it is dark or light.
    pub(super) fn theme_items(&self, list: &[(String, Theme)], query: &str) -> Vec<Item> {
        // searched by name only, as the tailnet's rows are (the action's name would match each)
        let rows: Vec<Item> = list
            .iter()
            .map(|(n, t)| Item {
                name: n.clone(),
                title: n.clone(),
                keywords: vec![],
                shortcut: Some(if t.is_dark() { "dark" } else { "light" }.into()),
                args: Some(json!({ "theme": n })),
            })
            .collect();
        let found = crate::palette::filter(&rows, query).into_iter();
        found.map(|i| Item { name: "client.theme.change".into(), ..rows[i].clone() }).collect()
    }

    /// What the theme list sets: both appearances when the setting names one theme for both, else
    /// the side macOS is on.
    pub(super) fn theme_prompt(&self) -> &'static str {
        match (&self.themes.names, self.dark) {
            ((d, l), _) if d == l => "Theme for both appearances…",
            (_, true) => "Theme for when macOS is dark…",
            (_, false) => "Theme for when macOS is light…",
        }
    }

    /// Keep theme `name`: drawn at once (the server's reload agrees once it lands), and written to
    /// init.lua's `theme` as the Settings window writes it, for the side `theme_prompt` names. A
    /// refused write goes back and reopens the list with the reason.
    fn keep_theme(&mut self, name: String, cx: &mut Context<Self>) {
        let t = match load_theme(&self.themes_dir(), &name) {
            Ok(t) => t,
            Err(e) => return self.open_themes(Some(e), cx),
        };
        let Some(config) = self.config_path() else { return };
        let before = (self.look.dark, self.look.light, self.themes.names.clone());
        let (d, l) = self.themes.names.clone();
        let value = match (d == l, self.dark) {
            (true, _) => json!(name),
            (false, true) => json!({ "dark": name, "light": l }),
            (false, false) => json!({ "dark": d, "light": name }),
        };
        if d == l || self.dark {
            (self.look.dark, self.themes.names.0) = (t, name.clone());
        }
        if d == l || !self.dark {
            (self.look.light, self.themes.names.1) = (t, name);
        }
        cx.notify();
        cx.spawn(async move |this, cx| {
            let written = cx.background_executor().spawn(async move { write_setting(&config, "theme", &value) }).await;
            this.update(cx, |ws, cx| match written {
                Ok(_) => ws.reload_config(),
                Err(e) => {
                    (ws.look.dark, ws.look.light, ws.themes.names) = before;
                    ws.open_themes(Some(e), cx);
                }
            })
        })
        .detach();
    }
}
