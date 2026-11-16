//! What the app draws with from init.lua's settings (`server.settings`): a theme for each macOS
//! appearance, resolved from its name, and the terminal font.

use std::path::Path;

use leon_lua::settings::{load_theme, Settings, FADE_MAX};
use serde_json::Value;

use crate::theme::{Theme, DARK, LIGHT};

#[derive(Debug, Clone, PartialEq)]
pub struct Look {
    pub dark: Theme,
    pub light: Theme,
    pub font_family: String,
    pub font_size: f32,
    /// How far the panes without the focus are faded (0 to 0.8; 0 is off).
    pub fade: f32,
}

impl Default for Look {
    fn default() -> Self {
        Look::resolve(&Value::Null, Path::new(""))
    }
}

impl Look {
    /// `settings` (a `server.settings` reply's `settings`), the theme names looked up in `themes`.
    /// What is missing is the default; a name that no longer loads is the built-in theme for its
    /// appearance.
    pub fn resolve(settings: &Value, themes: &Path) -> Look {
        let d = Settings::default();
        let theme = |side: &str, default: &str, fallback: Theme| {
            let name = settings["theme"][side].as_str().unwrap_or(default);
            load_theme(themes, name).unwrap_or_else(|e| {
                eprintln!("leon: theme.{side}: {e}; drawing with the built-in one");
                fallback
            })
        };
        Look {
            dark: theme("dark", &d.theme.dark, DARK),
            light: theme("light", &d.theme.light, LIGHT),
            font_family: settings["font"]["family"].as_str().unwrap_or(&d.font_family).to_string(),
            font_size: settings["font"]["size"].as_f64().unwrap_or(d.font_size) as f32,
            // clamped: the reply is another process's, and a fade past the range can't be read
            fade: settings["fade_inactive_panes"].as_f64().unwrap_or(d.fade_inactive_panes).clamp(0., FADE_MAX) as f32,
        }
    }

    /// The theme for macOS's appearance now.
    pub fn theme(&self, dark: bool) -> Theme {
        if dark {
            self.dark
        } else {
            self.light
        }
    }
}
