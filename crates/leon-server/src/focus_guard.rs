//! The permission guard's focus half (docs/architecture/attention.md, "Only a person answers
//! permission prompts"): a person's next keys, typed where they were looking, must not land on a
//! block held for a person, or they would answer its prompt.
//!
//! The rule is leon-core's `State::keep_keys_off_held`, run after every change under the state
//! lock: the keys only move onto a held block when a person moves them there on purpose. Here is
//! what tells it that: `PERSON_MOVES`, set around a request of a person's (the app in-process, a
//! client outside every leon terminal, another machine) that isn't a close (`block.close`,
//! `session.close_window`), and carried to the Lua thread for the bindings and actions such a
//! request runs. A change made anywhere else (a program inside leon, a pane closing or exiting, a
//! person's close) has the keys moved off the held block again.
//!
//! `onto_held` adds a clear refusal where the request itself names where the keys land:
//! `session.focus_block`, `float_block`, `dock_block`, `unfloat_block` (each focuses the block),
//! `focus_window`, `zoom_block`, and `swap_blocks` (the focus stays with the place). `guard`
//! (agents.rs) answers those from a program inside leon with `permission_pending`.

use std::future::Future;

use leon_core::model::{Session, State};
use serde_json::Value;

tokio::task_local! {
    /// Whether the change running now is a person's own focus move. Unset outside a request.
    static PERSON_MOVES: bool;
}

/// Whether the change running now may put the keys on a held block (unset: no).
pub(crate) fn person_moves() -> bool {
    asked().unwrap_or(false)
}

/// What the request running now set, if one did.
pub(crate) fn asked() -> Option<bool> {
    PERSON_MOVES.try_with(|m| *m).ok()
}

/// Run `f` (a request's synchronous part, or a Lua job) as a person's focus move or not.
pub(crate) fn within<T>(moves: bool, f: impl FnOnce() -> T) -> T {
    PERSON_MOVES.sync_scope(moves, f)
}

/// The same for a request's deferred part.
pub(crate) fn scope<F: Future>(moves: bool, f: F) -> impl Future<Output = F::Output> {
    PERSON_MOVES.scope(moves, f)
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
