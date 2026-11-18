//! `server.settings` and the `settings_changed` event's payload: init.lua's settings as the API
//! shows them. lua.rs registers the method and emits the event on a reload that changes them.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, JsonSchema)]
pub struct SettingsView {
    /// The effective settings: init.lua's `leon.set` calls over the defaults.
    pub settings: Effective,
    /// Where each setting was last set, as `file:line`, by key (`font.size`); a setting at its
    /// default isn't listed.
    pub sources: std::collections::BTreeMap<String, String>,
}

/// The shape of `leon_lua::Settings::to_json`, which builds it.
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct Effective {
    pub theme: ThemeChoice,
    pub font: Font,
    pub notifications: Notifications,
    /// The app copies a selection to the clipboard as it is made.
    pub copy_on_select: bool,
    /// Milliseconds after a repeatable binding during which its keys work without the prefix
    /// (0 turns repeat off).
    pub repeat_time: u64,
    /// Regular expressions hints match besides the built-in ones.
    pub hint_patterns: Vec<String>,
    /// What a restored block does with its program's resume command: `ask`, `always` or `never`.
    pub resume: String,
    /// How far the app fades the panes without the focus toward the window's background, from 0
    /// (off) to 0.8.
    pub fade_inactive_panes: f64,
    /// Leon.app looks for a newer release once a day; false, only when asked.
    pub check_for_updates: bool,
    /// How much of the desktop shows through Leon.app's window, from 0 (opaque) to 0.8.
    pub translucency: f64,
    /// The server also listens on this machine's Tailscale addresses, letting in only your own
    /// machines (`server.status`'s `tailnet` says how it went).
    pub serve_tailscale: bool,
    /// The port it listens on there.
    pub serve_tailscale_port: u16,
    /// Tagged machines it lets in too, by tag (`tag:ci`).
    pub serve_tailscale_tags: Vec<String>,
    /// With serve_tailscale on, the tailnet listener also serves the web page.
    pub serve_web: bool,
    /// The certificate file `tailscale cert` wrote, for HTTPS ("" for plain HTTP).
    pub serve_web_cert: String,
    /// Its private key file.
    pub serve_web_key: String,
}

/// A theme name for each OS appearance (the same one for both unless init.lua gives two).
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct ThemeChoice {
    pub dark: String,
    pub light: String,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct Font {
    pub family: String,
    /// Points, from 8 to 32.
    pub size: f64,
}

/// Which notifications the app posts.
#[derive(Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct Notifications {
    /// A long run finished while the app was in the background.
    pub done: bool,
    /// A block became blocked.
    pub blocked: bool,
    /// A block reported an error.
    pub error: bool,
}

/// The view of a loaded config's settings. The debug check makes a setting added to `to_json` but
/// not here fail every `server.settings` test instead of vanishing from the API.
pub(crate) fn view(cfg: &leon_lua::Config) -> SettingsView {
    let json = cfg.settings.to_json();
    let settings: Effective = serde_json::from_value(json.clone()).expect("Settings::to_json has Effective's shape");
    debug_assert_eq!(serde_json::to_value(&settings).ok(), Some(json), "Effective drifted from Settings::to_json");
    SettingsView { settings, sources: cfg.setting_sources.clone() }
}
