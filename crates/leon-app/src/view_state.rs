//! What tests and the other views read of the workspace: the selection, its windows and screens,
//! and the stopped-server state.

use super::*;

/// A GPUI rectangle as a settings frame.
pub(super) fn frame(b: Bounds<Pixels>) -> chrome::Frame {
    chrome::Frame { x: b.origin.x.into(), y: b.origin.y.into(), w: b.size.width.into(), h: b.size.height.into() }
}

impl Workspace {
    /// This Mac's server stopped from outside, and the window says so.
    pub fn server_stopped(&self) -> bool {
        self.stopped
    }

    /// Leon.app's updater, whose notice the window draws.
    pub fn updater(&self) -> Arc<crate::updates::Updater> {
        self.updates.clone()
    }

    /// How many sessions the sidebar lists, over every machine.
    pub fn session_count(&self) -> usize {
        self.rows().len()
    }

    /// What Start It Again ran, in a test workspace.
    pub fn relaunched(&self) -> Option<Vec<String>> {
        self.relaunched.clone()
    }

    /// Stand in for the native key event `key_down` reads (tests).
    pub fn set_native_key(&mut self, read: fn() -> Option<(u16, String)>) {
        self.native_key = read;
    }

    /// The selected session, if any.
    pub fn session_id(&self) -> Option<String> {
        self.session_id.clone()
    }

    /// How many times the poll has run, and how many of those asked for a redraw.
    pub fn polls(&self) -> (u64, u64) {
        self.polls
    }

    /// How many times the window rendered.
    pub fn renders(&self) -> u64 {
        self.renders
    }

    /// How many times what render draws from was brought up to date (`prepare`).
    pub fn prepares(&self) -> u64 {
        self.prepares
    }

    /// How many history fetches the panes asked for, one already out included.
    pub fn fetches(&self) -> u64 {
        self.term.fetches
    }

    pub fn sidebar_visible(&self) -> bool {
        self.sidebar
    }

    /// The block input goes to (`target`) in the selected session's active window.
    pub fn target_block(&self) -> Option<String> {
        self.target().map(|(_, id, _)| id)
    }

    /// The selected session's active window, as the app last heard it from the server.
    pub fn active_window(&self) -> Option<leon_core::Window> {
        let m = self.app.model();
        let s = m.session(self.session_id.as_deref()?)?;
        s.windows.get(s.active).cloned()
    }

    /// The selected session's windows in tab order, and which is active.
    pub fn windows(&self) -> Option<(Vec<leon_core::Window>, usize)> {
        let m = self.app.model();
        let s = m.session(self.session_id.as_deref()?)?;
        Some((s.windows.clone(), s.active))
    }

    /// The text of a block's screen as the app last received it (None before any screen came).
    pub fn screen_text(&self, block_id: &str) -> Option<String> {
        self.app.model().screens.get(block_id).map(|f| f.text())
    }
}

/// The selection: the target block, and the session kept live.
impl Workspace {
    /// The target block and its screen (an empty one until the first arrives: keys typed by then
    /// still go to the program).
    pub(super) fn target_screen(&self) -> Option<(String, String, Arc<render::Frame>)> {
        let (sid, id, _) = self.target()?;
        let f = self.app.model().screens.get(&id).cloned().unwrap_or_default();
        Some((sid, id, f))
    }

    /// (session id, target block id, whether it is zoomed) of the selected session's active window.
    pub(super) fn target(&self) -> Option<(String, String, bool)> {
        let sid = self.session_id.clone()?;
        let m = self.app.model();
        let s = m.session(&sid)?;
        let w = s.windows.get(s.active)?;
        Some((sid, target(w)?.to_string(), w.zoomed.is_some()))
    }

    /// Keep the selection on a live session: one that ended (closed here, its shell exited, or
    /// killed from the CLI, or its host went away) hands over to the session now in its place.
    pub(super) fn settle(&mut self, rows: &[SessionRow]) {
        if let Some(i) = rows.iter().position(|r| Some(&r.id) == self.session_id.as_ref()) {
            self.selected_at = i;
            self.use_host(rows[i].host);
            return;
        }
        self.selected_at = self.selected_at.min(rows.len().saturating_sub(1));
        self.session_id = rows.get(self.selected_at).map(|r| r.id.clone());
        self.use_host(rows.get(self.selected_at).map_or(0, |r| r.host));
    }
}
