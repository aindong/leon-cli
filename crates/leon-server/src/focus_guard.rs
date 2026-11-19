//! The permission guard's focus half (docs/architecture/attention.md, "Only a person answers
//! permission prompts"): a program inside leon must not move the focus onto a block held for a
//! person, or a person's next keys, typed where they were looking, would answer its prompt. `guard`
//! (agents.rs) refuses these requests from programs inside leon; a person may make any of them.
//!
//! Exact where the focus a request lands on is known from the request: `session.focus_block`,
//! `focus_window`, `zoom_block`, `float_block`, and `swap_blocks` (the focus stays with the place,
//! so swapping the focused pane with a held one puts the keys on the prompt). Conservative where
//! it depends on the layout or on where a closed float gives its focus back: `focus_direction`,
//! `focus_next_pane`, `focus_previous_pane` (refused while the active window holds a prompt that
//! isn't where the keys go now), and `session.toggle`, `close_window` of the active tab, `block.close` and
//! `session.move_block` of a block the keys go to (refused while the session holds such a prompt).
//! Let through: what keeps the focused block as it is (`dock_block`, `unfloat_block`,
//! `resize_pane`, labels), what focuses a new block (`new_split`, `new_window`,
//! `new_floating`, `session.create`: a new block can't be held), and `session.destroy` (the server
//! has no active session: each client picks what it shows next).

use leon_core::model::{Session, State};
use serde_json::Value;

/// Where a window's keys go: its zoomed block, else its focused one.
fn target(s: &Session, w: usize) -> Option<&str> {
    let w = s.windows.get(w)?;
    w.zoomed.as_deref().or(w.focus.as_deref())
}

/// The held block a request would put the focus on (or could, for the conservative ones), if any.
pub(crate) fn onto_held(st: &State, method: &str, params: &Value) -> Option<String> {
    let s = st.session(params["session_id"].as_str()?).ok()?;
    let now = target(s, s.active).map(str::to_string);
    let held =
        |id: &str| Some(id) != now.as_deref() && s.blocks.iter().any(|b| b.id == id && b.terminal.held_for_person());
    // the prompts the keys don't go to now: none, and nothing can be steered onto one
    let waiting: Vec<&str> = s.blocks.iter().map(|b| b.id.as_str()).filter(|id| held(id)).collect();
    let first = waiting.first().map(|id| id.to_string())?;
    let block = || params["block_id"].as_str();
    let window_of = |id: &str| s.window_of(id).ok();
    let is_target = |id: &str| window_of(id).is_some_and(|w| target(s, w) == Some(id));
    match method {
        "session.focus_block" | "session.float_block" => block().filter(|b| held(b)).map(str::to_string),
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
        "session.focus_direction" | "session.focus_next_pane" | "session.focus_previous_pane" => {
            let active = s.windows.get(s.active)?;
            waiting
                .iter()
                .find(|id| s.blocks.iter().any(|b| b.id == **id && b.window_id == active.id))
                .map(|id| id.to_string())
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
        "session.toggle" => Some(first),
        "session.close_window" => (params["window_id"].as_str()? == s.windows.get(s.active)?.id).then_some(first),
        "block.close" | "session.move_block" => {
            let b = block()?;
            (is_target(b) || held(b)).then(|| waiting.iter().find(|id| **id != b).map(|id| id.to_string())).flatten()
        }
        _ => None,
    }
}
