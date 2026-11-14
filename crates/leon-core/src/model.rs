//! Server state: an ordered list of sessions, each with ordered windows (tabs).

use crate::target::{resolve, Candidate};
use crate::{new_id, ApiError};

#[derive(Debug, Clone)]
pub struct Window {
    pub id: String,
    pub label: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Session {
    pub id: String,
    pub label: Option<String>,
    pub windows: Vec<Window>,
    /// Index into `windows` of the active tab.
    pub active: usize,
}

#[derive(Debug, Default)]
pub struct State {
    pub sessions: Vec<Session>,
}

/// Labels are optional, trimmed, and at most 128 characters (docs/spec.md §1).
pub fn clean_label(label: Option<&str>) -> Result<Option<String>, ApiError> {
    match label.map(str::trim) {
        None | Some("") => Ok(None),
        Some(l) if l.chars().count() > 128 => Err(ApiError::bad_request("labels are at most 128 characters")),
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
        self.sessions.push(Session { id: ids.0.clone(), label, windows: vec![window], active: 0 });
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

    /// Closing the last window destroys its session.
    pub fn close_window(&mut self, session_id: &str, window_id: &str) -> Result<(), ApiError> {
        let s = self.session_mut(session_id)?;
        let i = s.window_index(window_id)?;
        s.windows.remove(i);
        if s.windows.is_empty() {
            return self.destroy_session(session_id);
        }
        if s.active >= i && s.active > 0 {
            s.active -= 1;
        }
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

    #[test]
    fn labels_are_bounded() {
        assert_eq!(clean_label(Some(&"x".repeat(129))).unwrap_err().code, "bad_request");
        assert_eq!(clean_label(Some("  ")).unwrap(), None);
    }
}
