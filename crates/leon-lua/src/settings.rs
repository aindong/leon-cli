//! The settings `leon.set` takes (docs/lua/settings.md), typed and with their defaults; the one
//! check both `leon.set` and `leon config set` use; and the themes a setting can name: leon's
//! built-in ones and the files in `themes/` beside init.lua.

use std::path::{Path, PathBuf};

use leon_core::terminal::{MAX_SCROLLBACK, SCROLLBACK};
use leon_core::theme::{self, Theme};
use serde_json::{json, Value};

use crate::Resume;

/// Every key `leon.set` takes.
pub const KEYS: [&str; 17] = [
    "theme",
    "font.family",
    "font.size",
    "notifications.done",
    "notifications.blocked",
    "notifications.error",
    "copy_on_select",
    "repeat_time",
    "hint_patterns",
    "resume",
    "fade_inactive_panes",
    "check_for_updates",
    "translucency",
    "serve_tailscale",
    "serve_tailscale_port",
    "serve_tailscale_tags",
    "scrollback",
];

/// How far the app fades the panes without the focus toward the window's background, unless the
/// config says: the strongest fade that keeps body text at 4.5:1 on both built-in themes.
pub const FADE: f64 = 0.35;
/// The strongest fade a config can ask for: past it a faded pane can't be read.
pub const FADE_MAX: f64 = 0.8;

/// How much of the desktop shows through Leon.app's window, unless the config says (the app
/// keeps text readable over any desktop whatever it is; docs/lua/settings.md).
pub const TRANSLUCENCY: f64 = 0.3;
/// The most a config can ask for: past it the chrome is hardly there.
pub const TRANSLUCENCY_MAX: f64 = 0.8;

/// The port the server listens on over the tailnet unless the config says (docs/cli/hosts.md).
pub const TAILNET_PORT: u16 = 7297;
/// The most tags `serve_tailscale_tags` holds.
pub const MOST_TAGS: usize = 16;

/// The app's own font size range (it clamps ⌘+ and ⌘- to it too).
pub const FONT_SIZES: (f64, f64) = (8., 32.);

/// The theme for each OS appearance; one name is the same for both.
#[derive(Debug, Clone, PartialEq)]
pub struct ThemeChoice {
    pub dark: String,
    pub light: String,
}

/// The app's notifications (docs/architecture/attention.md), by kind.
#[derive(Debug, Clone, PartialEq)]
pub struct Notifications {
    /// A long run finished while the app was in the background.
    pub done: bool,
    /// A block became blocked.
    pub blocked: bool,
    /// A block reported an error.
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
    /// Milliseconds after a repeatable binding during which its keys work without the prefix; 0
    /// turns repeat off.
    pub repeat_time: u64,
    /// Regular expressions hints match besides the built-in ones.
    pub hint_patterns: Vec<String>,
    /// What a restored block does with its program's resume command.
    pub resume: Resume,
    /// How far the app fades the panes without the focus toward the window's background, 0 to
    /// `FADE_MAX`; 0 turns the fade off.
    pub fade_inactive_panes: f64,
    /// Leon.app looks for a newer release once a day (docs/updates.md); false: only when asked.
    pub check_for_updates: bool,
    /// How much of the desktop shows through the app's window, 0 (opaque) to `TRANSLUCENCY_MAX`.
    pub translucency: f64,
    /// The server also listens on this machine's Tailscale addresses, for your own machines only.
    pub serve_tailscale: bool,
    /// The port it listens on there.
    pub serve_tailscale_port: u16,
    /// Tagged machines let in besides yours: tags such as `tag:ci`.
    pub serve_tailscale_tags: Vec<String>,
    /// Lines of history each terminal keeps, 0 to `MAX_SCROLLBACK`.
    pub scrollback: usize,
}

/// tmux's `repeat-time` default, in milliseconds, for a config that doesn't set `repeat_time`.
pub const REPEAT_TIME: u64 = 500;

impl Default for Settings {
    /// Today's app: the dark theme whatever the OS, Menlo 13, every notification, 500 ms repeat,
    /// unfocused panes faded by `FADE`, a daily look for a new release, a translucent window.
    fn default() -> Self {
        let dark = "leon-dark".to_string();
        Settings {
            theme: ThemeChoice { light: dark.clone(), dark },
            font_family: "Menlo".into(),
            font_size: 13.,
            notifications: Notifications { done: true, blocked: true, error: true },
            copy_on_select: false,
            repeat_time: REPEAT_TIME,
            hint_patterns: vec![],
            resume: Resume::Ask,
            fade_inactive_panes: FADE,
            check_for_updates: true,
            translucency: TRANSLUCENCY,
            serve_tailscale: false,
            serve_tailscale_port: TAILNET_PORT,
            serve_tailscale_tags: vec![],
            scrollback: SCROLLBACK,
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
            "repeat_time": self.repeat_time,
            "hint_patterns": self.hint_patterns,
            "resume": self.resume.as_str(),
            "fade_inactive_panes": self.fade_inactive_panes,
            "check_for_updates": self.check_for_updates,
            "translucency": self.translucency,
            "serve_tailscale": self.serve_tailscale,
            "serve_tailscale_port": self.serve_tailscale_port,
            "serve_tailscale_tags": self.serve_tailscale_tags,
            "scrollback": self.scrollback,
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
            "check_for_updates" => self.check_for_updates = flag()?,
            "repeat_time" => match v.as_f64() {
                Some(ms) if ms.fract() == 0. && (0. ..=10_000.).contains(&ms) => self.repeat_time = ms as u64,
                _ => return Err("repeat_time is whole milliseconds, 0 to 10000 (0 turns repeat off)".into()),
            },
            "hint_patterns" => self.hint_patterns = hint_patterns(v)?,
            "resume" => {
                let r = Resume::ALL.into_iter().find(|r| v.as_str() == Some(r.as_str()));
                self.resume = r.ok_or("resume must be \"ask\", \"always\" or \"never\"")?;
            }
            "fade_inactive_panes" => match v.as_f64() {
                Some(a) if (0. ..=FADE_MAX).contains(&a) => self.fade_inactive_panes = a,
                _ => return Err(format!("fade_inactive_panes must be a number from 0 (off) to {FADE_MAX}")),
            },
            "translucency" => match v.as_f64() {
                Some(a) if (0. ..=TRANSLUCENCY_MAX).contains(&a) => self.translucency = a,
                _ => return Err(format!("translucency must be a number from 0 (opaque) to {TRANSLUCENCY_MAX}")),
            },
            "serve_tailscale" => self.serve_tailscale = flag()?,
            // a port and nothing else: the addresses come from Tailscale, never from a setting
            "serve_tailscale_port" => match v.as_f64() {
                Some(p) if p.fract() == 0. && (1. ..=65535.).contains(&p) => self.serve_tailscale_port = p as u16,
                _ => return Err("serve_tailscale_port is a port, a whole number from 1 to 65535".into()),
            },
            "serve_tailscale_tags" => self.serve_tailscale_tags = tags(v)?,
            "scrollback" => match v.as_f64() {
                Some(n) if n.fract() == 0. && (0. ..=MAX_SCROLLBACK as f64).contains(&n) => {
                    self.scrollback = n as usize
                }
                _ => return Err(format!("scrollback is whole lines of history, 0 to {MAX_SCROLLBACK}")),
            },
            _ if key.starts_with("notifications.") => {
                return Err(format!("unknown setting {key:?}; the notification kinds are done, blocked and error"))
            }
            _ => return Err(format!("unknown setting {key:?}; the settings are {}", KEYS.join(", "))),
        }
        Ok(())
    }
}

/// A list of at most `MAX_PATTERNS` strings, each a regular expression leon_core::hints compiles.
fn hint_patterns(v: &Value) -> Result<Vec<String>, String> {
    use leon_core::hints::{compile, MAX_PATTERNS};
    let list = match v {
        Value::Array(list) => list.as_slice(),
        Value::Object(o) if o.is_empty() => &[], // Lua's `{}`
        Value::Object(_) => {
            return Err("hint_patterns must be a list of regular expressions, not a table with keys".into())
        }
        v => return Err(format!("hint_patterns must be a list of regular expressions, got {}", kind(v))),
    };
    if list.len() > MAX_PATTERNS {
        return Err(format!("at most {MAX_PATTERNS} hint patterns, got {}", list.len()));
    }
    list.iter()
        .map(|p| {
            let p = p.as_str().ok_or("hint_patterns holds strings only")?;
            compile(p)?;
            Ok(p.to_string())
        })
        .collect()
}

/// At most `MOST_TAGS` Tailscale tags, each `tag:` and a name of letters, digits and `-`.
fn tags(v: &Value) -> Result<Vec<String>, String> {
    let bad = || format!("serve_tailscale_tags is a list of at most {MOST_TAGS} tags such as \"tag:ci\"");
    let list = match v {
        Value::Array(list) => list.as_slice(),
        Value::Object(o) if o.is_empty() => &[],
        _ => return Err(bad()),
    };
    if list.len() > MOST_TAGS {
        return Err(bad());
    }
    let tag = |t: &str| {
        t.strip_prefix("tag:").is_some_and(|n| {
            !n.is_empty() && n.len() <= 64 && n.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
        })
    };
    list.iter().map(|t| t.as_str().filter(|t| tag(t)).map(str::to_string).ok_or_else(bad)).collect()
}

/// A JSON value's type as Lua names it, for messages.
fn kind(v: &Value) -> &'static str {
    match v {
        Value::Null => "nil",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) | Value::Object(_) => "table",
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
    let read = theme::import::read_file(&path);
    match read {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            Err(format!("no theme {name:?}; `leon theme list` shows them, `leon theme import` adds one"))
        }
        Err(e) => Err(format!("can't read {}: {e}", path.display())),
        Ok(bytes) => theme::import::parse(&bytes).map_err(|e| format!("{}: {e}", path.display())),
    }
}
