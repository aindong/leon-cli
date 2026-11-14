//! Server state: an ordered list of sessions, each with ordered windows (tabs), each window a
//! layout tree of the terminal blocks in it. Every mutation records its events (events.rs).

use std::collections::VecDeque;
use std::sync::Arc;

use serde_json::json;

use crate::events::Event;
use crate::layout::{Direction, Node, Side};
use crate::target::{resolve, Candidate};
use crate::terminal::Terminal;
use crate::{new_id, ApiError};

/// How many exit statuses of closed blocks are remembered for `wait`.
const EXITS_KEPT: usize = 1024;

#[derive(Debug, Clone)]
pub struct Window {
    pub id: String,
    pub label: Option<String>,
    /// None only while the window's first block is being opened.
    pub layout: Option<Node>,
    pub focus: Option<String>,
    /// The block shown filling the window (display only; the layout stays).
    pub zoomed: Option<String>,
}

impl Window {
    fn new(label: Option<String>) -> Window {
        Window { id: new_id("window"), label, layout: None, focus: None, zoomed: None }
    }

    /// Take block `id` out of the tree, collapsing its split; focus moves to its sibling.
    pub(crate) fn detach(&mut self, id: &str) {
        let next = self.layout.as_ref().and_then(|t| t.sibling_leaf(id)).map(str::to_string);
        self.layout = self.layout.take().and_then(|t| t.without(id));
        if self.focus.as_deref() == Some(id) {
            self.focus = next;
        }
        if self.zoomed.as_deref() == Some(id) {
            self.zoomed = None;
        }
    }
}

/// Where a new block goes in a window that already has blocks.
#[derive(Debug, Clone)]
pub struct Place {
    /// A block or split in the window.
    pub anchor: String,
    pub direction: Direction,
    pub side: Side,
    /// The before side's share.
    pub ratio: f64,
    /// Focus the new block.
    pub focus: bool,
}

/// A terminal in a window. Dropping the last handle to its `terminal` hangs up the program.
#[derive(Debug, Clone)]
pub struct Block {
    pub id: String,
    pub label: Option<String>,
    pub window_id: String,
    /// Stays open after its program exits (`exit = { on_completion = false }`).
    pub keep_open: bool,
    pub exit_code: Option<i32>,
    pub terminal: Arc<Terminal>,
}

#[derive(Debug, Clone)]
pub struct Session {
    pub id: String,
    pub label: Option<String>,
    pub windows: Vec<Window>,
    /// Index into `windows` of the active tab.
    pub active: usize,
    /// In creation order, which is the order block positions count in.
    pub blocks: Vec<Block>,
    /// Bumped by every change to what `session.view` shows.
    pub revision: u64,
}

#[derive(Debug, Default)]
pub struct State {
    pub sessions: Vec<Session>,
    /// Exit statuses of finished programs, newest last, so `wait` works after a block closed.
    exits: VecDeque<(String, i32)>,
    /// Events not yet taken (events.rs).
    pub(crate) outbox: Vec<Event>,
    /// Sessions whose view changed since the events were last taken.
    pub(crate) dirty: Vec<String>,
}

/// Labels are optional, trimmed, and at most 128 characters (docs/spec.md §1).
pub fn clean_label(label: Option<&str>) -> Result<Option<String>, ApiError> {
    match label.map(str::trim) {
        None | Some("") => Ok(None),
        Some(l) if l.chars().count() > 128 => Err(ApiError::bad_request("labels are at most 128 characters")),
        // Labels get printed to terminals; control characters would let a program inject escapes.
        Some(l) if l.chars().any(char::is_control) => {
            Err(ApiError::bad_request("labels can't contain control characters"))
        }
        Some(l) => Ok(Some(l.to_string())),
    }
}

impl Session {
    pub fn window_index(&self, window_id: &str) -> Result<usize, ApiError> {
        self.windows
            .iter()
            .position(|w| w.id == window_id)
            .ok_or_else(|| ApiError::not_found(format!("no window {window_id} in {}", self.id)))
    }

    /// The index of block `block_id` in `blocks`.
    pub fn block_index(&self, block_id: &str) -> Result<usize, ApiError> {
        self.blocks
            .iter()
            .position(|b| b.id == block_id)
            .ok_or_else(|| ApiError::not_found(format!("no block {block_id} in {}", self.id)))
    }

    /// The index of the window holding block `block_id`.
    pub fn window_of(&self, block_id: &str) -> Result<usize, ApiError> {
        let i = self.block_index(block_id)?;
        self.window_index(&self.blocks[i].window_id)
    }
}

impl State {
    pub fn session(&self, id: &str) -> Result<&Session, ApiError> {
        self.sessions.iter().find(|s| s.id == id).ok_or_else(|| ApiError::session_not_found(id))
    }

    pub fn session_mut(&mut self, id: &str) -> Result<&mut Session, ApiError> {
        self.sessions.iter_mut().find(|s| s.id == id).ok_or_else(|| ApiError::session_not_found(id))
    }

    /// A new session with one window; returns (session id, window id).
    pub fn create_session(
        &mut self,
        label: Option<&str>,
        window_label: Option<&str>,
    ) -> Result<(String, String), ApiError> {
        let label = clean_label(label)?;
        let window = Window::new(clean_label(window_label)?);
        let ids = (new_id("session"), window.id.clone());
        let session = json!({ "session_id": ids.0, "label": label });
        let s = Session { id: ids.0.clone(), label, windows: vec![window], active: 0, blocks: vec![], revision: 0 };
        self.sessions.push(s);
        self.emit_server(&ids.0, "session_created", json!({ "session": session }));
        Ok(ids)
    }

    pub fn destroy_session(&mut self, id: &str) -> Result<(), ApiError> {
        let i = self.sessions.iter().position(|s| s.id == id).ok_or_else(|| ApiError::session_not_found(id))?;
        self.sessions.remove(i);
        self.emit_server(id, "session_destroyed", json!({ "session_id": id }));
        Ok(())
    }

    /// A new window at the end of the session's tabs, made active.
    pub fn new_window(&mut self, session_id: &str, label: Option<&str>) -> Result<String, ApiError> {
        let label = clean_label(label)?;
        let s = self.session_mut(session_id)?;
        let w = Window::new(label.clone());
        let id = w.id.clone();
        s.windows.push(w);
        s.active = s.windows.len() - 1;
        self.emit(session_id, "window_created", json!({ "window_id": id, "label": label }));
        self.emit(session_id, "active_window_changed", json!({ "window_id": id }));
        self.touch(session_id);
        Ok(id)
    }

    /// Closing a window closes its blocks; closing the last window destroys its session.
    pub fn close_window(&mut self, session_id: &str, window_id: &str) -> Result<(), ApiError> {
        let s = self.session_mut(session_id)?;
        let i = s.window_index(window_id)?;
        let was_active = s.windows[s.active].id.clone();
        s.windows.remove(i);
        let closed: Vec<String> = s.blocks.iter().filter(|b| b.window_id == window_id).map(|b| b.id.clone()).collect();
        s.blocks.retain(|b| b.window_id != window_id);
        if s.windows.is_empty() {
            return self.destroy_session(session_id);
        }
        if s.active >= i && s.active > 0 {
            s.active -= 1;
        }
        let active = s.windows[s.active].id.clone();
        for b in closed {
            self.emit(session_id, "block_closed", json!({ "block_id": b }));
        }
        self.emit(session_id, "window_closed", json!({ "window_id": window_id }));
        if active != was_active {
            self.emit(session_id, "active_window_changed", json!({ "window_id": active }));
        }
        self.touch(session_id);
        Ok(())
    }

    /// Add a block to its window: as the whole layout of an empty window (`place` None), or beside
    /// an anchor in a window that has blocks.
    pub fn add_block(&mut self, session_id: &str, block: Block, place: Option<&Place>) -> Result<(), ApiError> {
        let s = self.session_mut(session_id)?;
        let w = s.window_index(&block.window_id)?;
        let win = &mut s.windows[w];
        let id = block.id.clone();
        match (&mut win.layout, place) {
            (None, None) => {
                win.layout = Some(Node::block(&id));
                win.focus = Some(id.clone());
            }
            (Some(tree), Some(p)) => {
                if !tree.insert(&p.anchor, &id, p.direction, p.side, p.ratio) {
                    return Err(ApiError::not_found(format!("no block or split {} in {}", p.anchor, win.id)));
                }
                if p.focus || win.focus.is_none() {
                    win.focus = Some(id.clone());
                }
            }
            _ => return Err(ApiError::bad_request("an empty window takes no anchor; a window with blocks needs one")),
        }
        s.blocks.push(block);
        self.emit(session_id, "block_created", json!({ "block_id": id }));
        self.touch(session_id);
        Ok(())
    }

    /// The window (by id) whose layout holds `anchor`, a block or split id.
    pub fn anchor_window(&self, session_id: &str, anchor: &str) -> Result<String, ApiError> {
        let s = self.session(session_id)?;
        let w = s.windows.iter().find(|w| w.layout.as_ref().is_some_and(|t| t.contains(anchor)));
        Ok(w.ok_or_else(|| ApiError::not_found(format!("no block or split {anchor} in {session_id}")))?.id.clone())
    }

    /// Close a block, collapsing its split; a window left without blocks closes too.
    pub fn close_block(&mut self, session_id: &str, block_id: &str) -> Result<(), ApiError> {
        let s = self.session_mut(session_id)?;
        let i = s.blocks.iter().position(|b| b.id == block_id).ok_or_else(|| no_block(block_id))?;
        let w = s.window_of(block_id)?;
        s.blocks.remove(i);
        s.windows[w].detach(block_id);
        let emptied = s.windows[w].layout.is_none().then(|| s.windows[w].id.clone());
        self.emit(session_id, "block_closed", json!({ "block_id": block_id }));
        match emptied {
            Some(window_id) => self.close_window(session_id, &window_id),
            None => {
                self.touch(session_id);
                Ok(())
            }
        }
    }

    /// A block's program exited: remember the status, and close the block unless it is kept open.
    pub fn block_exited(&mut self, block_id: &str, code: i32) {
        self.exits.push_back((block_id.to_string(), code));
        if self.exits.len() > EXITS_KEPT {
            self.exits.pop_front();
        }
        self.emit_block(block_id, "child_exited", json!({ "exit_code": code }));
        let Some(s) = self.sessions.iter_mut().find(|s| s.blocks.iter().any(|b| b.id == block_id)) else {
            return; // closed already, e.g. its session was killed
        };
        let b = s.blocks.iter_mut().find(|b| b.id == block_id).expect("found above");
        b.exit_code = Some(code);
        if !b.keep_open {
            let sid = s.id.clone();
            let _ = self.close_block(&sid, block_id);
        }
    }

    /// The exit status of a block's program, if it has exited (also after the block closed).
    pub fn exit_status(&self, block_id: &str) -> Option<i32> {
        let kept_open = || self.block(block_id).ok().and_then(|(_, b)| b.exit_code);
        self.exits.iter().rev().find(|(id, _)| id == block_id).map(|&(_, code)| code).or_else(kept_open)
    }

    /// The session holding a block, and the block.
    pub fn block(&self, block_id: &str) -> Result<(&Session, &Block), ApiError> {
        self.sessions
            .iter()
            .find_map(|s| s.blocks.iter().find(|b| b.id == block_id).map(|b| (s, b)))
            .ok_or_else(|| no_block(block_id))
    }

    /// Resolve a block in one session, or across every session when `session_id` is None.
    /// Returns (session id, block id).
    pub fn resolve_block(&self, session_id: Option<&str>, query: &str) -> Result<(String, String), ApiError> {
        let sessions = match session_id {
            Some(id) => std::slice::from_ref(self.session(id)?),
            None => &self.sessions[..],
        };
        let all: Vec<(&Session, &Block)> = sessions.iter().flat_map(|s| s.blocks.iter().map(move |b| (s, b))).collect();
        let items: Vec<Candidate> =
            all.iter().map(|(_, b)| Candidate { id: &b.id, label: b.label.as_deref() }).collect();
        let (s, b) = all[resolve("block", query, &items)?];
        Ok((s.id.clone(), b.id.clone()))
    }

    pub fn set_session_label(&mut self, session_id: &str, label: &str) -> Result<(), ApiError> {
        let label = clean_label(Some(label))?;
        self.session_mut(session_id)?.label = label.clone();
        self.emit(session_id, "session_label_changed", json!({ "label": label }));
        self.touch(session_id);
        Ok(())
    }

    pub fn set_window_label(&mut self, session_id: &str, window_id: &str, label: &str) -> Result<(), ApiError> {
        let label = clean_label(Some(label))?;
        let s = self.session_mut(session_id)?;
        let i = s.window_index(window_id)?;
        s.windows[i].label = label.clone();
        self.emit(session_id, "window_label_changed", json!({ "window_id": window_id, "label": label }));
        self.touch(session_id);
        Ok(())
    }

    pub fn resolve_session(&self, query: &str) -> Result<&Session, ApiError> {
        let items: Vec<Candidate> =
            self.sessions.iter().map(|s| Candidate { id: &s.id, label: s.label.as_deref() }).collect();
        Ok(&self.sessions[resolve("session", query, &items)?])
    }

    pub fn resolve_window(&self, session_id: &str, query: &str) -> Result<&Window, ApiError> {
        let s = self.session(session_id)?;
        let items: Vec<Candidate> =
            s.windows.iter().map(|w| Candidate { id: &w.id, label: w.label.as_deref() }).collect();
        Ok(&s.windows[resolve("window", query, &items)?])
    }
}

fn no_block(id: &str) -> ApiError {
    ApiError::not_found(format!("no block {id}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sessions_start_with_one_window_and_close_cleans_up() {
        let mut st = State::default();
        let (s, w0) = st.create_session(Some(" web "), None).unwrap();
        assert_eq!(st.session(&s).unwrap().label.as_deref(), Some("web"), "labels are trimmed");
        let w1 = st.new_window(&s, Some("logs")).unwrap();
        assert_eq!(st.session(&s).unwrap().active, 1, "a new window becomes active");
        st.close_window(&s, &w1).unwrap();
        assert_eq!(st.session(&s).unwrap().active, 0);
        st.close_window(&s, &w0).unwrap();
        assert!(st.sessions.is_empty(), "closing the last window destroys the session");
    }

    fn block(id: &str, window_id: &str, keep_open: bool) -> Block {
        let spec = crate::terminal::Spec { command: vec!["sleep".into(), "300".into()], cwd: None, env: vec![] };
        let terminal = Arc::new(Terminal::spawn(spec, |_| {}).unwrap());
        Block { id: id.into(), label: None, window_id: window_id.into(), keep_open, exit_code: None, terminal }
    }

    #[test]
    fn exits_close_blocks_unless_kept_open() {
        let mut st = State::default();
        let (s, w0) = st.create_session(None, None).unwrap();
        let w1 = st.new_window(&s, None).unwrap();
        st.add_block(&s, block("block:a", &w0, false), None).unwrap();
        st.add_block(&s, block("block:b", &w1, true), None).unwrap();
        st.block_exited("block:b", 2);
        assert_eq!(st.block("block:b").unwrap().1.exit_code, Some(2), "kept open");
        st.block_exited("block:a", 3);
        assert!(st.block("block:a").is_err());
        assert_eq!(st.exit_status("block:a"), Some(3), "remembered after closing");
        assert_eq!(st.session(&s).unwrap().windows.len(), 1, "its emptied window closed");
        st.close_block(&s, "block:b").unwrap();
        assert!(st.sessions.is_empty(), "the last block's window was the last window");
    }

    #[test]
    fn labels_are_bounded() {
        assert_eq!(clean_label(Some(&"x".repeat(129))).unwrap_err().code, "bad_request");
        assert_eq!(clean_label(Some("  ")).unwrap(), None);
    }
}
