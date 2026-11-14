//! The settings `leon.set` takes (docs/lua.md "Settings"), typed and with their defaults; the one
//! check both `leon.set` and `leon config set` use; and the themes a setting can name: leon's
//! built-in ones and the files in `themes/` beside init.lua.

use std::path::{Path, PathBuf};

use leon_core::theme::{self, Theme};
use serde_json::{json, Value};

use crate::Resume;

/// Every key `leon.set` takes.
pub const KEYS: [&str; 8] = [
    "theme",
    "font.family",
    "font.size",
    "notifications.done",
    "notifications.blocked",
    "notifications.error",
    "copy_on_select",
    "resume",
];

/// The app's own font size range (it clamps ⌘+ and ⌘- to it too).
pub const FONT_SIZES: (f64, f64) = (8., 32.);

/// The theme for each OS appearance; one name is the same for both.
#[derive(Debug, Clone, PartialEq)]
pub struct ThemeChoice {
    pub dark: String,
    pub light: String,
}

/// Which program states may post a system notification.
#[derive(Debug, Clone, PartialEq)]
pub struct Notifications {
    pub done: bool,
    pub blocked: bool,
    pub error: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Settings {
    pub theme: ThemeChoice,
    pub font_family: String,
    pub font_size: f64,
    pub notifications: Notifications,
    /// The app copies a selection to the clipboard as it is made.
    pub copy_on_select: bool,
    /// What a restored block does with its program's resume command.
    pub resume: Resume,
}

impl Default for Settings {
    /// Today's app: the dark theme whatever the OS, Menlo 13, every notification.
    fn default() -> Self {
        let dark = "leon-dark".to_string();
        Settings {
            theme: ThemeChoice { light: dark.clone(), dark },
            font_family: "Menlo".into(),
            font_size: 13.,
            notifications: Notifications { done: true, blocked: true, error: true },
            copy_on_select: false,
            resume: Resume::Ask,
        }
    }
}

impl Settings {
    /// All of them, as `server.settings` reports them.
    pub fn to_json(&self) -> Value {
        let n = &self.notifications;
        json!({
            "theme": { "dark": self.theme.dark, "light": self.theme.light },
            "font": { "family": self.font_family, "size": self.font_size },
            "notifications": { "done": n.done, "blocked": n.blocked, "error": n.error },
            "copy_on_select": self.copy_on_select,
            "resume": self.resume.as_str(),
        })
    }

    /// One key's value (`font.size` is `13.0`); None for a key that isn't a setting.
    pub fn get(&self, key: &str) -> Option<Value> {
        if !KEYS.contains(&key) {
            return None;
        }
        key.split('.').try_fold(self.to_json(), |v, part| v.get(part).cloned())
    }

    /// Set `key` to `v`, or say why not. A theme name must be built in or a file in `themes`.
    pub fn set(&mut self, key: &str, v: &Value, themes: &Path) -> Result<(), String> {
        let flag = || v.as_bool().ok_or_else(|| format!("{key} must be true or false"));
        match key {
            "theme" => {
                let name = |v: &Value, what: &str| {
                    let n = v.as_str().ok_or_else(|| format!("{what} must be a theme name"))?;
                    load_theme(themes, n).map(|_| n.to_string())
                };
                self.theme = match v {
                    Value::Object(o) => {
                        if let Some(k) = o.keys().find(|k| *k != "dark" && *k != "light") {
                            return Err(format!("theme has no {k:?}; give {{ dark = NAME, light = NAME }}"));
                        }
                        let get =
                            |k| o.get(k).ok_or_else(|| "theme needs both dark and light, not just one".to_string());
                        ThemeChoice {
                            dark: name(get("dark")?, "theme.dark")?,
                            light: name(get("light")?, "theme.light")?,
                        }
                    }
                    Value::String(_) => {
                        let n = name(v, "theme")?;
                        ThemeChoice { dark: n.clone(), light: n }
                    }
                    _ => return Err("theme must be a theme name or { dark = NAME, light = NAME }".into()),
                };
            }
            "font.family" => match v.as_str().map(str::trim) {
                Some(f) if !f.is_empty() && f.len() <= 200 => self.font_family = f.to_string(),
                _ => return Err("font.family must be a font's name".into()),
            },
            "font.size" => match v.as_f64() {
                Some(s) if (FONT_SIZES.0..=FONT_SIZES.1).contains(&s) => self.font_size = s,
                _ => return Err(format!("font.size must be a number from {} to {}", FONT_SIZES.0, FONT_SIZES.1)),
            },
            "notifications.done" => self.notifications.done = flag()?,
            "notifications.blocked" => self.notifications.blocked = flag()?,
            "notifications.error" => self.notifications.error = flag()?,
            "copy_on_select" => self.copy_on_select = flag()?,
            "resume" => {
                let r = Resume::ALL.into_iter().find(|r| v.as_str() == Some(r.as_str()));
                self.resume = r.ok_or("resume must be \"ask\", \"always\" or \"never\"")?;
            }
            _ if key.starts_with("notifications.") => {
                return Err(format!("unknown setting {key:?}; the notification kinds are done, blocked and error"))
            }
            _ => return Err(format!("unknown setting {key:?}; the settings are {}", KEYS.join(", "))),
        }
        Ok(())
    }
}

/// `themes/` in the folder of the config file at `config`.
pub fn themes_dir(config: &Path) -> PathBuf {
    let dir = config.parent().filter(|d| !d.as_os_str().is_empty()).unwrap_or(Path::new("."));
    dir.join("themes")
}

/// A name a stored theme can have: a file name in `themes/`, and a word `leon config set theme`
/// can take (no `=` or `,`).
pub fn check_name(name: &str) -> Result<(), String> {
    let bad = name.is_empty()
        || name.len() > 64
        || name.starts_with('.')
        || name.chars().any(|c| c.is_control() || "/\\=,".contains(c));
    if bad {
        return Err(format!(
            "{name:?} can't name a theme: use up to 64 characters, no / \\ = or comma, not starting with ."
        ));
    }
    Ok(())
}

/// The built-in names, then the stored themes in `themes`, sorted.
pub fn theme_names(themes: &Path) -> Vec<String> {
    let mut stored: Vec<String> = std::fs::read_dir(themes)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| e.file_name().to_str()?.strip_suffix(".theme").map(str::to_string))
        .filter(|n| check_name(n).is_ok() && theme::builtin(n).is_none())
        .collect();
    stored.sort();
    theme::BUILTIN.iter().map(|(n, _)| n.to_string()).chain(stored).collect()
}

/// The theme called `name`: built in, else `themes/NAME.theme`.
pub fn load_theme(themes: &Path, name: &str) -> Result<Theme, String> {
    if let Some(t) = theme::builtin(name) {
        return Ok(t);
    }
    check_name(name)?;
    let path = themes.join(format!("{name}.theme"));
    let mut bytes = vec![];
    let read = std::fs::File::open(&path).and_then(|f| {
        use std::io::Read;
        f.take(theme::import::MAX_BYTES as u64 + 1).read_to_end(&mut bytes)
    });
    match read {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            Err(format!("no theme {name:?}; `leon theme list` shows them, `leon theme import` adds one"))
        }
        Err(e) => Err(format!("can't read {}: {e}", path.display())),
        Ok(_) => theme::import::parse(&bytes).map_err(|e| format!("{}: {e}", path.display())),
    }
}
