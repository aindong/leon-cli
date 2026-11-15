//! `client.keymap`'s reply: the loaded config as clients see it.

use super::*;

#[derive(Serialize, JsonSchema)]
pub struct Keymap {
    /// Bumped by every good load.
    pub generation: u64,
    pub path: Option<String>,
    /// Oldest first; for the same key the newest wins.
    pub bindings: Vec<BindingView>,
    pub modes: Vec<ModeView>,
    pub server_actions: Vec<ActionView>,
    /// `leon.unbind` calls, for a client to apply to its own default bindings.
    pub unbinds: Vec<UnbindView>,
    /// The config's client settings (`leon.set`).
    pub settings: ClientSettings,
}

#[derive(Serialize, JsonSchema, PartialEq)]
pub struct ClientSettings {
    /// Copy a selection to the clipboard as it is made (default false).
    pub copy_on_select: bool,
    /// Milliseconds after a repeatable binding (`repeats`) during which its keys work without the
    /// prefix (default 500; 0 turns repeat off).
    pub repeat_time: u64,
    /// Regular expressions hints match besides the built-in ones (group 1, if any, is the text).
    pub hint_patterns: Vec<String>,
    /// Which notifications the app posts (`notifications.*`, default all).
    pub notifications: crate::settings::Notifications,
}

#[derive(Serialize, JsonSchema, PartialEq)]
pub struct UnbindView {
    /// The chords, without the mode.
    pub key: String,
    pub mode: Option<String>,
    /// Only bindings to this action; none = every binding of the key.
    pub action: Option<String>,
}

#[derive(Serialize, JsonSchema, PartialEq)]
pub struct BindingView {
    /// The chords, without the mode (`ctrl+b>c`).
    pub key: String,
    pub mode: Option<String>,
    pub action: Option<String>,
    pub args: Option<Value>,
    /// Bound to a Lua function.
    #[serde(rename = "fn")]
    pub func: bool,
    /// After the sequence, its last key works again without the prefix for `repeat_time`.
    pub repeats: bool,
    /// `file:line`.
    pub source: String,
}

#[derive(Serialize, JsonSchema, PartialEq)]
pub struct ModeView {
    pub name: String,
    pub exclusive: bool,
    /// The chord that enters the mode for one key (`leon attach`'s prefix), if set.
    pub prefix: Option<String>,
    pub source: String,
}

#[derive(Serialize, JsonSchema)]
pub struct ActionView {
    pub name: String,
    pub title: String,
    pub description: Option<String>,
    pub category: String,
    pub keywords: Vec<String>,
    /// JSON Schema of the arguments.
    pub args: Option<Value>,
    pub repeats: bool,
    pub source: String,
}

#[derive(Serialize, JsonSchema)]
pub struct Reloaded {
    pub generation: u64,
    pub path: String,
    pub bindings: usize,
    pub modes: usize,
    pub actions: usize,
    /// Skipped calls and warnings, as `file:line: level: message`.
    pub diagnostics: Vec<String>,
    pub logs: Vec<String>,
}

pub(super) fn keymap(l: &Loaded) -> Keymap {
    let c = &l.cfg;
    Keymap {
        generation: l.generation,
        path: l.path.as_ref().map(|p| p.display().to_string()),
        bindings: c
            .bindings
            .iter()
            .map(|b| {
                let (action, args) = match &b.target {
                    Target::Action { name, args } => (Some(name.clone()), args.clone()),
                    Target::Function => (None, None),
                };
                let func = action.is_none();
                BindingView {
                    key: b.key.sequence(),
                    mode: b.key.mode.clone(),
                    action,
                    args,
                    func,
                    repeats: b.repeats,
                    source: b.source.clone(),
                }
            })
            .collect(),
        modes: c
            .modes
            .iter()
            .map(|m| ModeView {
                name: m.name.clone(),
                exclusive: m.exclusive,
                prefix: m.prefix.clone(),
                source: m.source.clone(),
            })
            .collect(),
        server_actions: c
            .actions
            .iter()
            .map(|a| ActionView {
                name: a.name.clone(),
                title: a.title.clone(),
                description: a.description.clone(),
                category: a.category.clone(),
                keywords: a.keywords.clone(),
                args: a.args.clone(),
                repeats: a.repeats,
                source: a.source.clone(),
            })
            .collect(),
        unbinds: c
            .unbinds
            .iter()
            .map(|u| UnbindView { key: u.key.sequence(), mode: u.key.mode.clone(), action: u.action.clone() })
            .collect(),
        settings: {
            let s = &c.settings;
            let n = &s.notifications;
            ClientSettings {
                copy_on_select: s.copy_on_select,
                repeat_time: s.repeat_time,
                hint_patterns: s.hint_patterns.clone(),
                notifications: crate::settings::Notifications { done: n.done, blocked: n.blocked, error: n.error },
            }
        },
    }
}
