//! The app's key bindings: built-in defaults with the config's (`client.keymap`) on top, chords
//! from GPUI keystrokes, and sequence resolution (`ctrl+b>c`).

#[cfg(all(feature = "gui", target_os = "macos"))]
use gpui::Keystroke;
use std::time::{Duration, Instant};

use leon_lua::keys::{parse, Chord, Key};
use serde_json::{json, Value};

/// Built-in shortcuts as (key, action). The config's bindings replace them key by key, and
/// `leon.unbind` removes them. Cmd+1..9 (`client.tab.goto`) are added by `Keymap::new`.
pub const DEFAULTS: &[(&str, &str)] = &[
    ("cmd+n", "session.new"),
    ("cmd+t", "client.tab.new"),
    ("cmd+d", "pane.split.right"),
    ("cmd+shift+d", "pane.split.down"),
    ("cmd+alt+left", "pane.focus.left"),
    ("cmd+alt+right", "pane.focus.right"),
    ("cmd+alt+up", "pane.focus.up"),
    ("cmd+alt+down", "pane.focus.down"),
    ("cmd+shift+enter", "pane.zoom"),
    ("cmd+w", "pane.close"),
    ("cmd+alt+f", "pane.float"),
    ("cmd+alt+shift+f", "pane.unfloat"),
    ("cmd+j", "pane.scratch.toggle"),
    ("cmd+ctrl+left", "pane.dock.left"),
    ("cmd+ctrl+right", "pane.dock.right"),
    ("cmd+ctrl+down", "pane.dock.bottom"),
    ("cmd+shift+]", "client.tab.next"),
    ("cmd+shift+[", "client.tab.previous"),
    ("cmd+v", "pane.paste"),
    ("cmd+b", "client.sidebar.toggle"),
    ("cmd+shift+s", "client.tab_style.toggle"),
    ("cmd+=", "client.font.increase"),
    ("cmd+shift+=", "client.font.increase"),
    ("cmd+-", "client.font.decrease"),
    ("cmd+0", "client.font.reset"),
    ("cmd+shift+p", "client.palette.toggle"),
    ("cmd+,", "client.settings"),
    ("cmd+k", "session.switch"),
    ("cmd+shift+a", "client.sidebar.activity.toggle"),
    ("cmd+\\", "client.overview.session.toggle"),
    ("cmd+shift+\\", "client.overview.toggle"),
    ("cmd+c", "pane.copy"),
    ("cmd+a", "pane.select_all"),
    ("cmd+f", "client.find.open"),
    ("cmd+g", "client.find.next"),
    ("cmd+shift+g", "client.find.previous"),
    ("cmd+shift+c", "client.copy_mode"),
    ("cmd+shift+h", "client.hints"),
    ("cmd+up", "client.scroll.top"),
    ("cmd+down", "client.scroll.bottom"),
];

/// Defaults only the app has: keys a terminal does send, which `leon attach` must pass to the
/// pane (it has no scrollback of its own to page).
pub const APP_DEFAULTS: &[(&str, &str)] =
    &[("shift+pageup", "client.scroll.page_up"), ("shift+pagedown", "client.scroll.page_down")];

/// A keystroke as a binding chord, written unshifted (docs/lua.md), or None for a key binding syntax
/// has no name for. shortcut: assumes a US layout for shifted symbols; physical keys (`[KeyA]`)
/// never match since GPUI keystrokes carry no key code.
#[cfg(all(feature = "gui", target_os = "macos"))]
pub fn chord(ks: &Keystroke) -> Option<Chord> {
    let m = &ks.modifiers;
    let mut mods = leon_lua::keys::Mods { cmd: m.platform, ctrl: m.control, alt: m.alt, shift: m.shift };
    let mut chars = ks.key.chars();
    let name = match (chars.next(), chars.next()) {
        // macOS reports cmd+shift+= as `+`
        (Some(c), None) => match leon_core::keys::unshifted(c) {
            Some(base) => {
                mods.shift = true;
                base.to_string()
            }
            None => c.to_lowercase().to_string(),
        },
        _ => ks.key.clone(),
    };
    let mut spec = parse(&name).ok().filter(|s| s.mode.is_none() && s.seq.len() == 1)?;
    Some(Chord { mods, key: spec.seq.remove(0).key })
}

/// A config key's chords, with shifted symbols written as typed (`ctrl+b>%`) made unshifted
/// (`shift+5`) so they match what `chord` makes of the keystroke.
fn config_seq(key: &str) -> Option<Vec<Chord>> {
    let mut seq = parse(key).ok()?.seq;
    for c in &mut seq {
        if let Key::Char(ch) = c.key {
            if let Some(base) = leon_core::keys::unshifted(ch) {
                c.mods.shift = true;
                c.key = Key::Char(base);
            }
        }
    }
    Some(seq)
}

/// One binding the app resolves: a client action, a config action, or a Lua function (no action).
#[derive(Debug, Clone, PartialEq)]
pub struct Bound {
    pub seq: Vec<Chord>,
    pub action: Option<String>,
    pub args: Option<Value>,
    pub from_config: bool,
    /// tmux `bind -r`: after it runs, its last key (or another repeatable one after the same
    /// prefix) works again without the prefix for `Keymap::repeat_time`.
    pub repeats: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Pressed {
    /// The keys so far complete this binding.
    Run(Bound),
    /// The keys so far start a longer sequence.
    Pending,
    Unbound,
}

/// A config action, for the palette.
#[derive(Debug, Clone, PartialEq)]
pub struct ConfigAction {
    pub name: String,
    pub title: String,
    pub keywords: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct Keymap {
    /// The server's keymap generation (0 until a config has loaded).
    pub generation: u64,
    bindings: Vec<Bound>,
    pub actions: Vec<ConfigAction>,
    /// `leon.set("copy_on_select", true)` in the config.
    pub copy_on_select: bool,
    /// How long a repeatable binding's keys work without the prefix (`leon.set("repeat_time")`).
    pub repeat_time: Duration,
    /// The config's own hint patterns (`leon.set("hint_patterns")`).
    pub hint_patterns: Vec<String>,
    /// The `notify_*` settings.
    pub notify: crate::attention::Settings,
}

impl Keymap {
    /// The defaults with a `client.keymap` reply on top. Mode bindings are left out: the app has
    /// no modes yet (nor does `client.key`).
    pub fn new(km: &Value) -> Keymap {
        Self::with_defaults(km, &[])
    }

    /// The app's keymap: `new`'s with `APP_DEFAULTS` too.
    pub fn app(km: &Value) -> Keymap {
        Self::with_defaults(km, APP_DEFAULTS)
    }

    fn with_defaults(km: &Value, extra: &[(&str, &str)]) -> Keymap {
        let goto = (1..=9).map(|i| (format!("cmd+{i}"), "client.tab.goto", Some(json!({ "index": i }))));
        let defaults = DEFAULTS.iter().chain(extra).map(|(k, a)| (k.to_string(), *a, None)).chain(goto);
        let mut bindings: Vec<Bound> = defaults
            .map(|(k, a, args)| Bound {
                seq: parse(&k).expect("a default key parses").seq,
                action: Some(a.into()),
                args,
                from_config: false,
                repeats: false,
            })
            .collect();
        let list = |k: &str| km[k].as_array().cloned().unwrap_or_default();
        let unmoded = |v: &Value| v["mode"].is_null();
        for u in list("unbinds").iter().filter(|u| unmoded(u)) {
            let Some(seq) = u["key"].as_str().and_then(config_seq) else { continue };
            let only = u["action"].as_str();
            bindings.retain(|b| b.seq != seq || only.is_some_and(|a| b.action.as_deref() != Some(a)));
        }
        for b in list("bindings").iter().filter(|b| unmoded(b)) {
            let Some(seq) = b["key"].as_str().and_then(config_seq) else { continue };
            // a key is either a binding or a prefix (the rule leon.bind keeps within the file)
            bindings.retain(|old| {
                let n = old.seq.len().min(seq.len());
                old.seq[..n] != seq[..n]
            });
            let action = b["action"].as_str().map(String::from);
            bindings.push(Bound {
                seq,
                action,
                args: b.get("args").filter(|a| !a.is_null()).cloned(),
                from_config: true,
                repeats: b["repeats"] == true,
            });
        }
        let actions = list("server_actions")
            .iter()
            .filter_map(|a| {
                Some(ConfigAction {
                    name: a["name"].as_str()?.into(),
                    title: a["title"].as_str()?.into(),
                    keywords: a["keywords"].as_array()?.iter().filter_map(|k| k.as_str().map(String::from)).collect(),
                })
            })
            .collect();
        let settings = &km["settings"];
        let on = |name: &str| settings["notifications"][name] != false; // on unless the config turned it off
        let notify = crate::attention::Settings { blocked: on("blocked"), error: on("error"), done: on("done") };
        Keymap {
            generation: km["generation"].as_u64().unwrap_or(0),
            bindings,
            actions,
            copy_on_select: settings["copy_on_select"] == true,
            notify,
            repeat_time: Duration::from_millis(settings["repeat_time"].as_u64().unwrap_or(leon_lua::REPEAT_TIME)),
            hint_patterns: settings["hint_patterns"]
                .as_array()
                .map(|a| a.iter().filter_map(|p| p.as_str().map(String::from)).collect())
                .unwrap_or_default(),
        }
    }

    /// Resolve `next` after the pending `prefix`.
    pub fn press(&self, prefix: &[Chord], next: &Chord) -> Pressed {
        let starts = |b: &&Bound| b.seq.len() > prefix.len() && b.seq[..prefix.len()] == *prefix;
        let at = prefix.len();
        let mut hits = self.bindings.iter().filter(starts).filter(|b| b.seq[at] == *next);
        match hits.clone().find(|b| b.seq.len() > at + 1) {
            Some(_) => Pressed::Pending,
            None => hits.next_back().map_or(Pressed::Unbound, |b| Pressed::Run(b.clone())),
        }
    }

    /// The repeatable binding `next` completes after `prefix`, if any.
    pub fn repeatable(&self, prefix: &[Chord], next: &Chord) -> Option<Bound> {
        match self.press(prefix, next) {
            Pressed::Run(b) if b.repeats => Some(b),
            _ => None,
        }
    }

    /// What can follow `prefix`: (next chord, what it runs), one row per chord.
    pub fn hints(&self, prefix: &[Chord]) -> Vec<(String, String)> {
        let mut out: Vec<(String, String)> = Vec::new();
        for b in self.bindings.iter().filter(|b| b.seq.len() > prefix.len() && b.seq[..prefix.len()] == *prefix) {
            let next = b.seq[prefix.len()].to_string();
            let what = match (b.seq.len() > prefix.len() + 1, &b.action) {
                (true, _) => "…".to_string(),
                (false, Some(a)) => a.clone(),
                (false, None) => "Lua function".to_string(),
            };
            match out.iter_mut().find(|(k, _)| *k == next) {
                Some(row) => row.1 = what, // the newest binding wins
                None => out.push((next, what)),
            }
        }
        out
    }

    /// The keys that run `action`, as the README writes them (`⌘⇧D`): the config's newest binding,
    /// else the first default.
    pub fn shortcut(&self, action: &str) -> Option<String> {
        let mut all = self.bindings.iter().filter(|b| b.action.as_deref() == Some(action) && b.args.is_none());
        let first = all.clone().next();
        let b = all.rfind(|b| b.from_config).or(first)?;
        Some(b.seq.iter().map(glyphs).collect::<Vec<_>>().join(" "))
    }
}

/// The window after a repeatable binding ran (tmux `bind -r`): until it closes, a key that
/// completes a repeatable binding after the same prefix runs it with no prefix typed. Any other
/// key, or one past the window, closes it and is handled as if no prefix had been typed.
#[derive(Debug, Default)]
pub struct Repeat(Option<(Vec<Chord>, Instant)>);

impl Repeat {
    /// A binding ran after `prefix`: a repeatable one with a prefix opens the window for `time`,
    /// anything else closes it.
    pub fn ran(&mut self, prefix: &[Chord], repeats: bool, time: Duration, now: Instant) {
        self.0 = (repeats && !prefix.is_empty() && !time.is_zero()).then(|| (prefix.to_vec(), now + time));
    }

    /// A key at `now`: what `find(prefix, key)` makes of it while the window is open, with the
    /// prefix. The window closes either way; the caller's `ran` opens it again.
    pub fn press<T>(
        &mut self,
        key: &Chord,
        now: Instant,
        find: impl FnOnce(&[Chord], &Chord) -> Option<T>,
    ) -> Option<(T, Vec<Chord>)> {
        let (prefix, _) = self.0.take().filter(|(_, until)| now <= *until)?;
        Some((find(&prefix, key)?, prefix))
    }
}

/// `cmd+shift+d` as ⌘⇧D.
fn glyphs(c: &Chord) -> String {
    let m = c.mods;
    let mut s: String = [(m.cmd, "⌘"), (m.ctrl, "⌃"), (m.alt, "⌥"), (m.shift, "⇧")]
        .iter()
        .filter(|(on, _)| *on)
        .map(|(_, g)| *g)
        .collect();
    match &c.key {
        Key::Char(ch) => s.extend(ch.to_uppercase()),
        Key::Named(n) => s.push_str(match *n {
            "enter" => "↩",
            "escape" => "⎋",
            "tab" => "⇥",
            "space" => "Space",
            "backspace" => "⌫",
            "delete" => "⌦",
            "left" => "←",
            "right" => "→",
            "up" => "↑",
            "down" => "↓",
            other => other,
        }),
        Key::Physical(p) => s.push_str(p),
    }
    s
}
