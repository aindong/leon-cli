//! Layout changes on a session's windows: move, swap, zoom, resize and focus. Each returns the
//! session's revision, bumped only when something changed.

use serde_json::json;

use crate::layout::{swapped, Direction, Side, Toward};
use crate::{ApiError, State};

impl State {
    /// Put `block_id` beside `anchor_id` (any window of the session); an emptied window closes.
    pub fn move_block(
        &mut self,
        session_id: &str,
        block_id: &str,
        anchor_id: &str,
        direction: Direction,
        side: Side,
        ratio: f64,
    ) -> Result<u64, ApiError> {
        if block_id == anchor_id {
            return Err(ApiError::bad_request("a block can't move beside itself"));
        }
        let s = self.session_mut(session_id)?;
        let (from, to, i) = (s.window_of(block_id)?, s.window_of(anchor_id)?, s.block_index(block_id)?);
        let (focus, zoomed) = (s.windows[from].focus.clone(), s.windows[from].zoomed.clone());
        s.windows[from].detach(block_id);
        if from == to {
            // It never left the window, so its focus and zoom stay.
            (s.windows[to].focus, s.windows[to].zoomed) = (focus, zoomed);
        }
        let tree = s.windows[to].layout.as_mut().expect("the anchor is in it");
        assert!(tree.insert(anchor_id, block_id, direction, side, ratio), "the anchor is in it");
        s.blocks[i].window_id = s.windows[to].id.clone();
        if s.windows[from].layout.is_none() {
            let emptied = s.windows[from].id.clone();
            self.close_window(session_id, &emptied)?;
        }
        self.touch(session_id);
        Ok(self.revision(session_id))
    }

    /// Blocks trade places, across windows too; each window's focus and zoom stay where they were.
    pub fn swap_blocks(&mut self, session_id: &str, a: &str, b: &str) -> Result<u64, ApiError> {
        let s = self.session_mut(session_id)?;
        let (ia, ib) = (s.block_index(a)?, s.block_index(b)?);
        if a == b {
            return Ok(self.revision(session_id));
        }
        for w in &mut s.windows {
            if let Some(tree) = &mut w.layout {
                tree.swap(a, b);
            }
            for id in [&mut w.focus, &mut w.zoomed].into_iter().flatten() {
                *id = swapped(id, a, b).to_string();
            }
        }
        let wa = std::mem::take(&mut s.blocks[ia].window_id);
        s.blocks[ia].window_id = std::mem::replace(&mut s.blocks[ib].window_id, wa);
        self.touch(session_id);
        Ok(self.revision(session_id))
    }

    /// Zoom a block (`Some(true)`), unzoom it (`Some(false)`) or toggle (`None`).
    pub fn zoom_block(&mut self, session_id: &str, block_id: &str, zoom: Option<bool>) -> Result<u64, ApiError> {
        let s = self.session_mut(session_id)?;
        let i = s.window_of(block_id)?;
        let w = &mut s.windows[i];
        let on = zoom.unwrap_or(w.zoomed.as_deref() != Some(block_id));
        let new = match (on, w.zoomed.as_deref() == Some(block_id)) {
            (true, _) => Some(block_id.to_string()),
            (false, true) => None,
            (false, false) => w.zoomed.clone(), // another block's zoom isn't ours to clear
        };
        if new != w.zoomed {
            w.zoomed = new;
            self.touch(session_id);
        }
        Ok(self.revision(session_id))
    }

    /// Set the ratio of the split between two blocks of one window.
    pub fn resize_pane(
        &mut self,
        session_id: &str,
        direction: Direction,
        before_id: &str,
        after_id: &str,
        ratio: f64,
    ) -> Result<u64, ApiError> {
        check_ratio(ratio)?;
        let s = self.session_mut(session_id)?;
        let w = s.window_of(before_id)?;
        if s.window_of(after_id)? != w {
            return Err(ApiError::bad_request(format!("{before_id} and {after_id} are in different windows")));
        }
        let tree = s.windows[w].layout.as_mut().expect("it holds both");
        tree.set_ratio(direction, before_id, after_id, ratio).map_err(ApiError::bad_request)?;
        self.touch(session_id);
        Ok(self.revision(session_id))
    }

    /// Focus a block and make its window the active one.
    pub fn focus_block(&mut self, session_id: &str, block_id: &str) -> Result<u64, ApiError> {
        let s = self.session_mut(session_id)?;
        let w = s.window_of(block_id)?;
        let window_changed = s.active != w;
        if !window_changed && s.windows[w].focus.as_deref() == Some(block_id) {
            return Ok(self.revision(session_id));
        }
        s.active = w;
        s.windows[w].focus = Some(block_id.to_string());
        let window_id = s.windows[w].id.clone();
        if window_changed {
            self.emit(session_id, "active_window_changed", json!({ "window_id": window_id }));
        }
        self.touch(session_id);
        Ok(self.revision(session_id))
    }

    /// Move focus in the active window by direction; returns (revision, focused block).
    pub fn focus_direction(&mut self, session_id: &str, toward: Toward, wrap: bool) -> Result<(u64, String), ApiError> {
        self.focus_with(session_id, |tree, from| tree.neighbor(from, toward, wrap).map(str::to_string))
    }

    /// Move focus to the next (or previous) block of the active window, in leaf order.
    pub fn focus_step(&mut self, session_id: &str, forward: bool, wrap: bool) -> Result<(u64, String), ApiError> {
        self.focus_with(session_id, |tree, from| tree.step(from, forward, wrap).map(str::to_string))
    }

    fn focus_with(
        &mut self,
        session_id: &str,
        pick: impl Fn(&crate::layout::Node, &str) -> Option<String>,
    ) -> Result<(u64, String), ApiError> {
        let s = self.session(session_id)?;
        let w = &s.windows[s.active];
        let (Some(tree), Some(from)) = (&w.layout, &w.focus) else {
            return Err(ApiError::not_found("the active window has no focused block"));
        };
        let to = pick(tree, from).unwrap_or_else(|| from.clone());
        Ok((self.focus_block(session_id, &to)?, to))
    }

    /// A session's revision; 0 once it is gone (a call that destroyed it).
    pub fn revision(&self, session_id: &str) -> u64 {
        self.session(session_id).map_or(0, |s| s.revision)
    }
}

/// Split ratios are strictly between 0 and 1.
pub fn check_ratio(ratio: f64) -> Result<(), ApiError> {
    if ratio > 0.0 && ratio < 1.0 {
        Ok(())
    } else {
        Err(ApiError::bad_request(format!("ratio must be between 0 and 1, not {ratio}")))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::layout::{Direction::*, Side::*, Toward};
    use crate::model::Place;
    use crate::terminal::{Spec, Terminal};
    use crate::{Block, State};

    fn add(st: &mut State, s: &str, w: &str, id: &str, anchor: Option<&str>) {
        let spec = Spec { command: vec!["sleep".into(), "300".into()], cwd: None, env: vec![] };
        let terminal = Arc::new(Terminal::spawn(spec, |_| {}).unwrap());
        let b = Block { id: id.into(), label: None, window_id: w.into(), keep_open: false, exit_code: None, terminal };
        let place =
            anchor.map(|a| Place { anchor: a.into(), direction: Horizontal, side: After, ratio: 0.5, focus: true });
        st.add_block(s, b, place.as_ref()).unwrap();
    }

    #[test]
    fn revisions_and_events_follow_changes() {
        let mut st = State::default();
        let (s, w) = st.create_session(None, None).unwrap();
        add(&mut st, &s, &w, "block:a", None);
        add(&mut st, &s, &w, "block:b", Some("block:a"));
        let names = |st: &mut State| -> Vec<String> { st.take_events().iter().map(|e| e.body.to_string()).collect() };
        let first = names(&mut st);
        assert_eq!(first.iter().filter(|e| e.contains("session_view_changed")).count(), 1, "one per batch: {first:?}");
        let r = st.session(&s).unwrap().revision;
        // The server takes the events after every call; a revision bumps once per call.
        assert_eq!(st.focus_direction(&s, Toward::Left, false).unwrap(), (r + 1, "block:a".into()));
        assert_eq!(names(&mut st).len(), 1, "a view change only");
        assert_eq!(st.focus_direction(&s, Toward::Left, false).unwrap(), (r + 1, "block:a".into()), "no-op");
        assert!(names(&mut st).is_empty(), "no-op, no event");
        assert_eq!(st.zoom_block(&s, "block:b", None).unwrap(), r + 2);
        names(&mut st);
        assert_eq!(st.zoom_block(&s, "block:b", Some(true)).unwrap(), r + 2, "already on");
        assert!(st.resize_pane(&s, Horizontal, "block:a", "block:b", 1.0).is_err());
        assert!(st.move_block(&s, "block:a", "block:a", Vertical, After, 0.5).is_err());
        st.move_block(&s, "block:b", "block:a", Vertical, Before, 0.5).unwrap();
        let w0 = &st.session(&s).unwrap().windows[0];
        assert_eq!((w0.zoomed.as_deref(), w0.focus.as_deref()), (Some("block:b"), Some("block:a")), "kept in-window");
        st.close_block(&s, "block:b").unwrap();
        assert_eq!(st.session(&s).unwrap().windows[0].zoomed, None, "a closed block's zoom goes");

        // A focused split in a background window brings that window forward.
        let w1 = st.new_window(&s, None).unwrap();
        add(&mut st, &s, &w1, "block:c", None);
        names(&mut st);
        add(&mut st, &s, &w, "block:d", Some("block:a"));
        assert_eq!(st.session(&s).unwrap().active, 0);
        assert!(names(&mut st).iter().any(|e| e.contains("active_window_changed")));
    }
}
