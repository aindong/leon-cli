//! Host keys for `leon attach`: crossterm's key events as the bytes a pane reads and as binding
//! chords, and the keys after the prefix (the `attach` mode's table).

use leon_app::keymap::{Keymap, Pressed};
use leon_lua::keys::{parse, Chord, Key};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde_json::Value;

/// What the keys after the prefix do by default; `leon.bind("attach/KEY", …)` replaces one and
/// `leon.unbind("attach/KEY")` removes one.
pub const DEFAULTS: &[(&str, &str)] = &[
    ("d", "client.detach"),
    ("left", "pane.focus.left"),
    ("right", "pane.focus.right"),
    ("up", "pane.focus.up"),
    ("down", "pane.focus.down"),
    ("h", "pane.focus.left"),
    ("l", "pane.focus.right"),
    ("k", "pane.focus.up"),
    ("j", "pane.focus.down"),
    ("%", "pane.split.right"),
    ("\"", "pane.split.down"),
    ("z", "pane.zoom"),
    ("c", "client.tab.new"),
    ("n", "client.tab.next"),
    ("p", "client.tab.previous"),
    ("s", "client.picker"),
];

pub const PREFIX: &str = "ctrl+b";

/// The binding-syntax name of a key event (`ctrl+alt+left`, `shift+tab`, `x`), or None for keys
/// binding syntax has no name for. Characters keep their case and shifted symbols stay as typed.
fn name(k: &KeyEvent) -> Option<(String, bool)> {
    let m = k.modifiers;
    let mut mods = String::new();
    for (on, word) in [(KeyModifiers::CONTROL, "ctrl+"), (KeyModifiers::ALT, "alt+")] {
        if m.contains(on) {
            mods.push_str(word);
        }
    }
    let (key, shift) = match k.code {
        KeyCode::Char(' ') => ("space".to_string(), m.contains(KeyModifiers::SHIFT)),
        // crossterm reports ctrl+\ ] ^ _ (0x1c–0x1f) as ctrl+4…7
        KeyCode::Char(c @ '4'..='7') if m.contains(KeyModifiers::CONTROL) => {
            (["\\", "]", "^", "_"][(c as u8 - b'4') as usize].to_string(), false)
        }
        KeyCode::Char(c) if c.is_uppercase() => (c.to_lowercase().to_string(), true),
        KeyCode::Char(c) => (c.to_string(), false), // a shifted symbol already says shift
        KeyCode::BackTab => ("tab".into(), true),
        KeyCode::F(n) => (format!("f{n}"), m.contains(KeyModifiers::SHIFT)),
        code => {
            let named = match code {
                KeyCode::Enter => "enter",
                KeyCode::Esc => "escape",
                KeyCode::Tab => "tab",
                KeyCode::Backspace => "backspace",
                KeyCode::Delete => "delete",
                KeyCode::Insert => "insert",
                KeyCode::Home => "home",
                KeyCode::End => "end",
                KeyCode::PageUp => "pageup",
                KeyCode::PageDown => "pagedown",
                KeyCode::Up => "up",
                KeyCode::Down => "down",
                KeyCode::Left => "left",
                KeyCode::Right => "right",
                _ => return None,
            };
            (named.to_string(), m.contains(KeyModifiers::SHIFT))
        }
    };
    if shift {
        mods.push_str("shift+");
    }
    Some((mods + &key, m.contains(KeyModifiers::CONTROL)))
}

/// The bytes the focused pane gets for a key: typed characters as typed, everything else
/// through the table `send-key` uses, with the pane's own cursor-key mode.
pub fn bytes(k: &KeyEvent, app_cursor: bool) -> Option<Vec<u8>> {
    let alt = k.modifiers.contains(KeyModifiers::ALT);
    if let KeyCode::Char(c) = k.code {
        let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
        // crossterm reports ctrl+\ ] ^ _ (0x1c–0x1f) as ctrl+4…7
        let byte = match (ctrl, c) {
            (false, _) => None,
            (true, '4'..='7') => Some(0x1c + (c as u8 - b'4')),
            (true, ' ') => Some(0), // NUL arrives as ctrl+space
            (true, c) => leon_core::keys::encode(&format!("ctrl+{}", c.to_ascii_lowercase()), false).ok()?.pop(),
        };
        let mut out = match byte {
            Some(b) => vec![b],
            None => c.to_string().into_bytes(),
        };
        if alt {
            out.insert(0, 0x1b);
        }
        return Some(out);
    }
    leon_core::keys::encode(&name(k)?.0, app_cursor).ok()
}

/// A config key's chord with shifted symbols written as typed (`%`) made unshifted (`shift+5`),
/// the form the app's keymap and `chord` compare.
fn normal(mut c: Chord) -> Chord {
    if let Key::Char(ch) = c.key {
        if let Some(base) = leon_core::keys::unshifted(ch) {
            (c.mods.shift, c.key) = (true, Key::Char(base));
        }
    }
    c
}

/// One chord in binding syntax (`ctrl+b`, `%`), or an error naming it.
pub fn one_chord(s: &str) -> Result<Chord, String> {
    match parse(s) {
        Ok(mut k) if k.mode.is_none() && k.seq.len() == 1 => Ok(normal(k.seq.remove(0))),
        Ok(_) => Err(format!("{s:?} is not one key")),
        Err(e) => Err(e),
    }
}

/// A key event as a binding chord, unshifted as the app's keymap expects.
pub fn chord(k: &KeyEvent) -> Option<Chord> {
    let (name, ctrl) = name(k)?;
    // ctrl+B and ctrl+b are the same byte; a terminal can't tell them apart
    one_chord(&if ctrl { name.to_lowercase() } else { name }).ok()
}

/// What a key after the prefix does.
#[derive(Debug, PartialEq)]
pub enum After {
    /// An action (with args) from the `attach` table.
    Run(String, Option<Value>),
    /// A config binding on `prefix>KEY` (complete, or the start of a longer sequence).
    Config(Pressed),
    /// The prefix again: type it.
    Literal,
    Nothing,
}

/// The prefix, its table and the config's other bindings.
pub struct Keys {
    pub prefix: Chord,
    /// The prefix as the status bar shows it (`ctrl+b`).
    pub prefix_name: String,
    /// The config's `attach/KEY` bindings (action, args, repeats), newest last.
    config_table: Vec<(Chord, String, Option<Value>, bool)>,
    defaults: Vec<(Chord, String)>,
    pub config: Keymap,
}

impl Keys {
    /// From a `client.keymap` reply and `--prefix`: the flag, else the config's
    /// `leon.mode("attach", { prefix })`, else ctrl+b.
    pub fn new(km: &Value, flag: Option<&str>) -> Result<Keys, String> {
        let list = |k: &str| km[k].as_array().cloned().unwrap_or_default();
        let attach = |v: &Value| v["mode"] == "attach";
        let from_config =
            list("modes").iter().find(|m| m["name"] == "attach").and_then(|m| m["prefix"].as_str().map(String::from));
        let prefix_name = flag.map(String::from).or(from_config).unwrap_or_else(|| PREFIX.into());
        let prefix = one_chord(&prefix_name).map_err(|e| format!("bad prefix: {e}"))?;
        if prefix.mods.cmd {
            return Err(format!("bad prefix {prefix_name:?}: a terminal never sends cmd"));
        }
        let gone: Vec<Chord> =
            list("unbinds").iter().filter(|u| attach(u)).filter_map(|u| one_chord(u["key"].as_str()?).ok()).collect();
        let defaults = DEFAULTS
            .iter()
            .map(|(k, a)| (one_chord(k).expect("a default key parses"), a.to_string()))
            .filter(|(c, _)| !gone.contains(c))
            .collect();
        let config_table = list("bindings")
            .iter()
            .filter(|b| attach(b))
            .filter_map(|b| {
                let action = b["action"].as_str()?.to_string();
                let args = b.get("args").filter(|a| !a.is_null()).cloned();
                Some((one_chord(b["key"].as_str()?).ok()?, action, args, b["repeats"] == true))
            })
            .collect();
        Ok(Keys { prefix, prefix_name, config_table, defaults, config: Keymap::new(km) })
    }

    /// The key after the prefix: the config's `attach/KEY`, else its `prefix>KEY` sequence, else
    /// the built-in table.
    pub fn after_prefix(&self, c: &Chord) -> After {
        if let Some((_, a, args, _)) = self.config_table.iter().rev().find(|(k, ..)| k == c) {
            return After::Run(a.clone(), args.clone());
        }
        match self.config.press(std::slice::from_ref(&self.prefix), c) {
            Pressed::Unbound => {}
            p => return After::Config(p),
        }
        if *c == self.prefix {
            return After::Literal;
        }
        match self.defaults.iter().find(|(k, _)| k == c) {
            Some((_, a)) => After::Run(a.clone(), None),
            None => After::Nothing,
        }
    }
}

impl Keys {
    /// What the key after the prefix does when its binding has `repeats` (the built-in table never
    /// does): it may run again without the prefix for `repeat_time`.
    pub fn repeatable(&self, c: &Chord) -> Option<After> {
        if let Some((_, a, args, repeats)) = self.config_table.iter().rev().find(|(k, ..)| k == c) {
            return repeats.then(|| After::Run(a.clone(), args.clone()));
        }
        let b = self.config.repeatable(std::slice::from_ref(&self.prefix), c)?;
        Some(After::Config(Pressed::Run(b)))
    }
}

#[cfg(test)]
#[path = "keys_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "m19_keys_tests.rs"]
mod m19_tests;
