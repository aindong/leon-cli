//! init.lua's look in the workspace: the theme for the macOS appearance, the terminal font, and
//! the palette every host resolves the panes' colours with (`block.watch`'s `palette`).

use super::*;

/// macOS is in dark mode.
pub fn is_dark(a: gpui::WindowAppearance) -> bool {
    matches!(a, gpui::WindowAppearance::Dark | gpui::WindowAppearance::VibrantDark)
}

impl Workspace {
    /// The theme everything is drawn with now.
    pub fn theme(&self) -> Theme {
        self.theme
    }

    pub fn font_family(&self) -> String {
        self.font_family.to_string()
    }

    pub fn copy_on_select(&self) -> bool {
        self.keymap.copy_on_select
    }

    pub fn notify_settings(&self) -> crate::attention::Settings {
        self.keymap.notify
    }

    /// The screen of `block_id` as the app last received it.
    pub fn screen(&self, block_id: &str) -> Option<Arc<render::Frame>> {
        self.app.model().screens.get(block_id).cloned()
    }

    /// The init.lua the local server loaded (else where it would look).
    pub fn config_path(&self) -> Option<PathBuf> {
        self.config_path.clone().or_else(leon_lua::config_path)
    }

    /// Draw with a `server.settings` reply: its themes resolved, its font (fitting every pane to
    /// a new one at the next render).
    pub(super) fn apply_settings(&mut self, reply: &Value) {
        let themes = self.config_path().map(|p| leon_lua::settings::themes_dir(&p)).unwrap_or_default();
        let look = Look::resolve(&reply["settings"], &themes);
        if look.font_size != self.look.font_size {
            self.font_size = look.font_size.clamp(FONT_SIZES.0, FONT_SIZES.1);
            self.refit = true;
        }
        if look.font_family != self.look.font_family {
            self.font_family = look.font_family.clone().into();
            self.refit = true;
        }
        self.look = look;
        self.use_theme();
        // a server older than the setting doesn't report it, and would have skipped a
        // `check_for_updates = false` as unknown: no look of the app's own until it does
        self.check_for_updates = reply["settings"]["check_for_updates"].as_bool();
        self.updates_looked = None; // a reload may have turned it on
        self.look_for_updates();
    }

    /// Show the Settings window: open it, or bring the open one forward.
    pub(super) fn open_settings(&mut self, cx: &mut Context<Self>) {
        let open = self.settings_window.is_some_and(|h| h.update(cx, |_, window, _| window.activate_window()).is_ok());
        if !open {
            let Some(config) = self.config_path() else { return };
            self.settings_window = crate::settings_window::SettingsWindow::open(config, self.hosts.local().clone(), cx);
        }
    }

    /// macOS switched between light and dark.
    pub fn set_appearance(&mut self, dark: bool, cx: &mut Context<Self>) {
        self.dark = dark;
        self.use_theme();
        cx.notify();
    }

    /// Draw with the theme for the appearance now; scrollback fetched in the old colours goes.
    fn use_theme(&mut self) {
        let t = self.look.theme(self.dark);
        if t != self.theme {
            self.theme = t;
            self.term.panes.values_mut().for_each(|p| p.cache.clear());
        }
        self.send_palette();
    }

    /// Have every host resolve its screens in the theme's colours (a host that just connected too).
    pub(super) fn send_palette(&self) {
        for (_, remote) in self.hosts.remotes() {
            remote.set_palette(&self.theme);
        }
    }
}
