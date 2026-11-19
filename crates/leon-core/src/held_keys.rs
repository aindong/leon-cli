//! A block held for a person (a permission prompt) never gets the keys as a side effect
//! (docs/agents.md, "Only the user answers permission prompts"): after each change, a session whose
//! key target (its active tab's zoomed block, else its focus) became such a block, by anything but
//! a person moving there on purpose, has the keys moved off it again.

use crate::model::{Session, State};

/// Where a session's keys go: its active tab's zoomed block, else that tab's focus.
pub fn key_target(s: &Session) -> Option<&str> {
    target(s, s.active)
}

fn target(s: &Session, w: usize) -> Option<&str> {
    let w = s.windows.get(w)?;
    w.zoomed.as_deref().or(w.focus.as_deref())
}

impl State {
    /// Run after every change, before its events go out. `person_moves`: the change is a person's
    /// own focus move (not a close), which may put the keys on a held block.
    pub fn keep_keys_off_held(&mut self, person_moves: bool) {
        for i in 0..self.sessions.len() {
            let s = &self.sessions[i];
            let now = key_target(s).map(str::to_string);
            let was = self.keys.get(&s.id).cloned();
            if now == was {
                continue;
            }
            let sid = s.id.clone();
            let onto_held = now.as_deref().is_some_and(|t| held(s, t));
            if let (false, true, Some(t)) = (person_moves, onto_held, now.as_deref()) {
                if let Some(to) = off_held(s, was.as_deref(), t) {
                    self.give_keys(&sid, &to);
                }
            }
            match key_target(&self.sessions[i]).map(str::to_string) {
                Some(t) => self.keys.insert(sid, t),
                None => self.keys.remove(&sid),
            };
        }
        let sessions = &self.sessions;
        self.keys.retain(|sid, _| sessions.iter().any(|s| &s.id == sid));
    }

    fn give_keys(&mut self, session_id: &str, block_id: &str) {
        if let Ok(s) = self.session_mut(session_id) {
            if let Ok(w) = s.window_of(block_id) {
                // a zoom on another block would keep the keys there
                if s.windows[w].zoomed.as_deref().is_some_and(|z| z != block_id) {
                    s.windows[w].zoomed = None;
                }
            }
        }
        let _ = self.focus_block(session_id, block_id);
        self.touch(session_id);
    }
}

fn held(s: &Session, id: &str) -> bool {
    s.blocks.iter().any(|b| b.id == id && b.terminal.held_for_person())
}

/// Where the keys go instead of held block `t`: back to the block that had them, else the nearest
/// pane in the tab they fell in that isn't held, else the nearest tab whose keys aren't on one.
/// None when every block left is held: a tab always keeps a key target.
fn off_held(s: &Session, was: Option<&str>, t: &str) -> Option<String> {
    let free = |id: &str| s.blocks.iter().any(|b| b.id == id) && !held(s, id);
    if let Some(p) = was.filter(|p| free(p)) {
        return Some(p.to_string());
    }
    if let Some(tree) = s.windows.get(s.active).and_then(|w| w.layout.as_ref()) {
        let near = tree.sibling_leaf(t).filter(|id| free(id));
        if let Some(id) = near.or_else(|| tree.leaves().into_iter().find(|id| free(id))) {
            return Some(id.to_string());
        }
    }
    (1..s.windows.len())
        .flat_map(|d| [s.active.checked_sub(d), Some(s.active + d)])
        .flatten()
        .find_map(|w| target(s, w).filter(|id| free(id)))
        .map(str::to_string)
}
