//! Server state: an ordered list of sessions, each with ordered windows (tabs) and the
//! terminal blocks in them.

use std::collections::VecDeque;
use std::sync::Arc;

use crate::target::{resolve, Candidate};
use crate::terminal::Terminal;
use crate::{new_id, ApiError};

/// How many exit statuses of closed blocks are remembered for `wait`.
const EXITS_KEPT: usize = 1024;

#[derive(Debug, Clone)]
pub struct Window {
    pub id: String,
    pub label: Option<String>,
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
}

#[derive(Debug, Default)]
pub struct State {
    pub sessions: Vec<Session>,
    /// Exit statuses of finished programs, newest last, so `wait` works after a block closed.
    exits: VecDeque<(String, i32)>,
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
        let window = Window { id: new_id("window"), label: clean_label(window_label)? };
        let ids = (new_id("session"), window.id.clone());
        self.sessions.push(Session { id: ids.0.clone(), label, windows: vec![window], active: 0, blocks: vec![] });
        Ok(ids)
    }

    pub fn destroy_session(&mut self, id: &str) -> Result<(), ApiError> {
        let i = self.sessions.iter().position(|s| s.id == id).ok_or_else(|| ApiError::session_not_found(id))?;
        self.sessions.remove(i);
        Ok(())
    }

    /// A new window at the end of the session's tabs, made active.
    pub fn new_window(&mut self, session_id: &str, label: Option<&str>) -> Result<String, ApiError> {
        let label = clean_label(label)?;
        let s = self.session_mut(session_id)?;
        let id = new_id("window");
        s.windows.push(Window { id: id.clone(), label });
        s.active = s.windows.len() - 1;
        Ok(id)
    }

    /// Closing a window closes its blocks; closing the last window destroys its session.
    pub fn close_window(&mut self, session_id: &str, window_id: &str) -> Result<(), ApiError> {
        let s = self.session_mut(session_id)?;
        let i = s.window_index(window_id)?;
        s.windows.remove(i);
        s.blocks.retain(|b| b.window_id != window_id);
        if s.windows.is_empty() {
            return self.destroy_session(session_id);
        }
        if s.active >= i && s.active > 0 {
            s.active -= 1;
        }
        Ok(())
    }

    /// Make a window the session's active tab.
    pub fn focus_window(&mut self, session_id: &str, window_id: &str) -> Result<(), ApiError> {
        let s = self.session_mut(session_id)?;
        s.active = s.window_index(window_id)?;
        Ok(())
    }

    pub fn add_block(&mut self, session_id: &str, block: Block) -> Result<(), ApiError> {
        let s = self.session_mut(session_id)?;
        s.window_index(&block.window_id)?;
        s.blocks.push(block);
        Ok(())
    }

    /// Close a block; a window left without blocks closes too.
    pub fn close_block(&mut self, session_id: &str, block_id: &str) -> Result<(), ApiError> {
        let s = self.session_mut(session_id)?;
        let i = s.blocks.iter().position(|b| b.id == block_id).ok_or_else(|| no_block(block_id))?;
        let window_id = s.blocks.remove(i).window_id;
        if s.blocks.iter().all(|b| b.window_id != window_id) {
            self.close_window(session_id, &window_id)?;
        }
        Ok(())
    }

    /// A block's program exited: remember the status, and close the block unless it is kept open.
    pub fn block_exited(&mut self, block_id: &str, code: i32) {
        self.exits.push_back((block_id.to_string(), code));
        if self.exits.len() > EXITS_KEPT {
            self.exits.pop_front();
        }
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
        self.session_mut(session_id)?.label = label;
        Ok(())
    }

    pub fn set_window_label(&mut self, session_id: &str, window_id: &str, label: &str) -> Result<(), ApiError> {
        let label = clean_label(Some(label))?;
        let s = self.session_mut(session_id)?;
        let i = s.window_index(window_id)?;
        s.windows[i].label = label;
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
        st.add_block(&s, block("block:a", &w0, false)).unwrap();
        st.add_block(&s, block("block:b", &w1, true)).unwrap();
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
