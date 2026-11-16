//! What tests and the other views read of the workspace: the selection, its windows and screens,
//! and the stopped-server state.

use super::*;

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

    /// How many redraws were shown at once for a key's echo (`show_now`).
    pub fn echo_shows(&self) -> u64 {
        self.echo_shows
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
