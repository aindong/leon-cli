//! The permission guard's focus half (docs/architecture/attention.md, "Only a person answers
//! permission prompts"): a person's next keys, typed where they were looking, must not land on a
//! block held for a person, or they would answer its prompt.
//!
//! The rule is leon-core's `State::keep_keys_off_held`, run after every change under the state
//! lock: the keys only move onto a held block when a person moves them there on purpose, or stay
//! on one where a person put them or typed. Here is what tells it that: `CALLER`, set around each
//! request (serve.rs from the guard's answer; the app, which calls in-process, is the person) and
//! carried to the Lua thread for the bindings and actions the request runs. A person's request
//! moves the keys on purpose when it is a focus move (`MOVES`); anything else (a program inside
//! leon, a pane closing or exiting, a person's close or move of a pane) is a side effect, and has
//! the keys moved off the held block again. A call made in-process for a program's request (its
//! Lua, or a method's own calls) goes through the guard as the program's own would (lib.rs).
//!
//! `onto_held` adds a clear refusal where the request itself names where the keys land:
//! `session.focus_block`, `float_block`, `dock_block`, `unfloat_block` (each focuses the block),
//! `focus_window`, `zoom_block`, and `swap_blocks` (the focus stays with the place). `guard`
//! (agents.rs) answers those from a program inside leon with `permission_pending`.

use std::future::Future;

use leon_core::model::{Session, State};
use serde_json::Value;

/// Who a request is for: a person or a program, and whether it moves the keys on purpose.
#[derive(Clone, Copy)]
pub(crate) struct Caller {
    pub person: bool,
    pub moves: bool,
}

/// A person's requests that move the keys on purpose: focus moves, a new block (which can't be
/// held), and the key bindings and actions they press (whose calls are each judged again).
const MOVES: &[&str] = &[
    "session.focus_block",
    "session.focus_window",
    "session.focus_direction",
    "session.focus_next_pane",
    "session.focus_previous_pane",
    "session.toggle",
    "session.zoom_block",
    "session.float_block",
    "session.unfloat_block",
    "session.dock_block",
    "session.new_split",
    "session.new_window",
    "session.new_floating",
    "session.create",
    "client.key",
    "client.action",
];

impl Caller {
    pub(crate) fn of(person: bool, method: &str) -> Caller {
        Caller { person, moves: person && MOVES.contains(&method) }
    }
}

tokio::task_local! {
    /// Who the request running now is for. Unset outside a request.
    static CALLER: Caller;
}

/// Whether the change running now may put the keys on a held block (unset: no).
pub(crate) fn person_moves() -> bool {
    caller().is_some_and(|c| c.moves)
}

/// Who the request running now is for, if one is running.
pub(crate) fn caller() -> Option<Caller> {
    CALLER.try_with(|c| *c).ok()
}

/// Run `f` (a request's synchronous part, or a Lua job) for `c`.
pub(crate) fn within<T>(c: Caller, f: impl FnOnce() -> T) -> T {
    CALLER.sync_scope(c, f)
}

/// The same for a request's deferred part.
pub(crate) fn scope<F: Future>(c: Caller, f: F) -> impl Future<Output = F::Output> {
    CALLER.scope(c, f)
}

/// Where a window's keys go: its zoomed block, else its focused one.
fn target(s: &Session, w: usize) -> Option<&str> {
    let w = s.windows.get(w)?;
    w.zoomed.as_deref().or(w.focus.as_deref())
}

/// The held block a request would put the keys on, if any.
pub(crate) fn onto_held(st: &State, method: &str, params: &Value) -> Option<String> {
    let s = st.session(params["session_id"].as_str()?).ok()?;
    let now = target(s, s.active).map(str::to_string);
    let held =
        |id: &str| Some(id) != now.as_deref() && s.blocks.iter().any(|b| b.id == id && b.terminal.held_for_person());
    let block = || params["block_id"].as_str();
    let window_of = |id: &str| s.window_of(id).ok();
    match method {
        "session.focus_block" | "session.float_block" | "session.dock_block" | "session.unfloat_block" => {
            block().filter(|b| held(b)).map(str::to_string)
        }
        "session.focus_window" => {
            let w = s.window_index(params["window_id"].as_str()?).ok()?;
            target(s, w).filter(|b| held(b)).map(str::to_string)
        }
        "session.zoom_block" => {
            let b = block()?;
            let w = window_of(b)?;
            let win = &s.windows[w];
            let on = match &params["zoom"] {
                Value::Bool(on) => *on,
                _ => win.zoomed.as_deref() != Some(b), // "toggle", the default
            };
            // as panes::zoom_block: unzooming another block leaves its zoom
            let zoomed = match (on, win.zoomed.as_deref() == Some(b)) {
                (true, _) => Some(b),
                (false, true) => None,
                (false, false) => win.zoomed.as_deref(),
            };
            let after = zoomed.or(win.focus.as_deref());
            after.filter(|a| w == s.active && held(a)).map(str::to_string)
        }
        "session.swap_blocks" => {
            let (a, b) = (block()?, params["other_block_id"].as_str()?);
            let after = match now.as_deref() {
                Some(t) if t == a => b,
                Some(t) if t == b => a,
                _ => return None,
            };
            held(after).then(|| after.to_string())
        }
        _ => None,
    }
}
