//! Key syntax (docs/lua.md): `[mode/]chord[>chord…]`, a chord being `mod+…+key`.

use std::fmt;

/// The key of a chord.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Key {
    /// One character, lowercased (`a`, `=`, `+`).
    Char(char),
    /// A named key (`enter`, `pageup`, `f1`).
    Named(&'static str),
    /// A layout-independent physical key, W3C `KeyboardEvent.code` (`KeyA`).
    /// shortcut: kept as written and not checked against the W3C list; normalize if apps send other casings.
    Physical(String),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Mods {
    pub cmd: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Chord {
    pub mods: Mods,
    pub key: Key,
}

/// A parsed binding key: an optional mode and a sequence of one or more chords.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct KeySpec {
    pub mode: Option<String>,
    pub seq: Vec<Chord>,
}

const NAMED: &[&str] = &[
    "enter",
    "escape",
    "tab",
    "space",
    "backspace",
    "delete",
    "left",
    "right",
    "up",
    "down",
    "home",
    "end",
    "pageup",
    "pagedown",
    "f1",
    "f2",
    "f3",
    "f4",
    "f5",
    "f6",
    "f7",
    "f8",
    "f9",
    "f10",
    "f11",
    "f12",
];

impl Mods {
    fn slot(&mut self, word: &str) -> Option<(&mut bool, &'static str)> {
        Some(match word.to_ascii_lowercase().as_str() {
            "cmd" | "command" | "super" | "meta" | "win" => (&mut self.cmd, "cmd"),
            "ctrl" | "control" => (&mut self.ctrl, "ctrl"),
            "alt" | "opt" | "option" => (&mut self.alt, "alt"),
            "shift" => (&mut self.shift, "shift"),
            _ => return None,
        })
    }
}

/// Mode names: `[a-z0-9_-]`, not empty.
pub fn valid_mode(name: &str) -> bool {
    !name.is_empty() && name.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-')
}

pub fn parse(s: &str) -> Result<KeySpec, String> {
    if s.is_empty() {
        return Err("empty key".into());
    }
    // A prefix before `/` with no `+` or `>` is a mode name; otherwise the `/` is a key (`ctrl+/`).
    let (mode, rest) = match s.split_once('/') {
        Some((m, r)) if !m.is_empty() && !m.contains(['+', '>']) => {
            if !valid_mode(m) {
                return Err(format!("bad mode name {m:?} in {s:?}: use [a-z0-9_-]"));
            }
            (Some(m.to_string()), r)
        }
        _ => (None, s),
    };
    let cs: Vec<char> = rest.chars().collect();
    let mut i = 0;
    let mut seq = Vec::new();
    loop {
        let mut mods = Mods::default();
        let key = loop {
            let w = word(&cs, &mut i).ok_or_else(|| format!("missing key in {s:?}"))?;
            if cs.get(i) != Some(&'+') {
                break parse_key(&w).map_err(|e| format!("{e} in {s:?}"))?;
            }
            i += 1;
            let (slot, name) = mods.slot(&w).ok_or_else(|| format!("unknown modifier {w:?} in {s:?}"))?;
            if *slot {
                return Err(format!("{name} given twice in {s:?}"));
            }
            *slot = true;
        };
        seq.push(Chord { mods, key });
        match cs.get(i) {
            None => break,
            Some('>') => i += 1,
            Some(c) => return Err(format!("unexpected {c:?} in {s:?}")),
        }
    }
    Ok(KeySpec { mode, seq })
}

/// One word at `i`: `[…]`, a run of ASCII letters/digits, or any single other character.
/// A `[` that ends the chord is the `[` key itself (`ctrl+[`, `ctrl+b>[`).
fn word(cs: &[char], i: &mut usize) -> Option<String> {
    let start = *i;
    let c = *cs.get(start)?;
    *i += 1;
    if c == '[' && !matches!(cs.get(*i), None | Some('>')) {
        match cs[start..].iter().position(|&c| c == ']') {
            Some(n) => *i = start + n + 1,
            None => *i = cs.len(),
        }
    } else if c.is_ascii_alphanumeric() {
        while cs.get(*i).is_some_and(|c| c.is_ascii_alphanumeric()) {
            *i += 1;
        }
    }
    Some(cs[start..*i].iter().collect())
}

fn parse_key(w: &str) -> Result<Key, String> {
    if let Some(code) = w.strip_prefix('[').filter(|c| !c.is_empty()) {
        return match code.strip_suffix(']') {
            Some(code) if !code.is_empty() && code.bytes().all(|b| b.is_ascii_alphanumeric()) => {
                Ok(Key::Physical(code.to_string()))
            }
            _ => Err(format!("bad physical key {w:?}: write it like [KeyA]")),
        };
    }
    if Mods::default().slot(w).is_some() {
        return Err(format!("missing key after modifier {w:?}"));
    }
    let lower = w.to_lowercase();
    if let Some(n) = NAMED.iter().find(|n| **n == lower) {
        return Ok(Key::Named(n));
    }
    let mut chars = lower.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) if !c.is_whitespace() => Ok(Key::Char(c)),
        _ => Err(format!("unknown key {w:?}")),
    }
}

impl KeySpec {
    /// The chords without the mode, e.g. `ctrl+b>c`.
    pub fn sequence(&self) -> String {
        self.seq.iter().map(Chord::to_string).collect::<Vec<_>>().join(">")
    }
}

impl fmt::Display for Chord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let m = self.mods;
        for (on, name) in [(m.cmd, "cmd+"), (m.ctrl, "ctrl+"), (m.alt, "alt+"), (m.shift, "shift+")] {
            if on {
                f.write_str(name)?;
            }
        }
        match &self.key {
            Key::Char(c) => write!(f, "{c}"),
            Key::Named(n) => f.write_str(n),
            Key::Physical(code) => write!(f, "[{code}]"),
        }
    }
}

impl fmt::Display for KeySpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(m) = &self.mode {
            write!(f, "{m}/")?;
        }
        f.write_str(&self.sequence())
    }
}
