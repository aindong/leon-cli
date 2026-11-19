//! The model a remote keeps: the server's sessions, with their tabs, layouts and blocks, and the
//! watched blocks' screens.

use super::*;

/// One session as the app shows it.
#[derive(Debug, Clone)]
pub struct Session {
    pub id: String,
    pub label: Option<String>,
    pub revision: u64,
    /// Index of the active window.
    pub active: usize,
    pub windows: Vec<Window>,
    pub blocks: Vec<Block>,
    /// Each window's floats and drawers, by window id (`session.view`'s `floating` and `docked`).
    pub overlays: HashMap<String, Overlays>,
    /// Each window's tile focused last, by window id, kept across refetches: the server keeps only
    /// the focus, which a float or drawer can have.
    pub tiled_focus: HashMap<String, String>,
}

impl Session {
    /// Window `id`'s floats (back to front) and drawers; none for a window without any.
    pub fn overlay(&self, id: &str) -> Overlays {
        self.overlays.get(id).cloned().unwrap_or_default()
    }

    /// The tile that names window `w`: the zoomed or focused one, else the tile focused last (the
    /// focus is a float's or drawer's), else its first; the focused overlay only when there is no tile.
    pub fn named_tile<'a>(&'a self, w: &'a Window) -> Option<&'a str> {
        let leaves = w.layout.as_ref().map(|t| t.leaves()).unwrap_or_default();
        let tile = |id: &&str| leaves.contains(id);
        w.zoomed
            .as_deref()
            .or(w.focus.as_deref())
            .filter(tile)
            .or_else(|| self.tiled_focus.get(&w.id).map(String::as_str).filter(tile))
            .or_else(|| leaves.first().copied())
            .or(w.focus.as_deref())
    }

    /// Remember each window's tile focused last, from this fetch's focus or the one before it.
    pub(super) fn remember_tiles(&mut self, before: Option<&Session>) {
        self.tiled_focus = self
            .windows
            .iter()
            .filter_map(|w| {
                let leaves = w.layout.as_ref().map(|t| t.leaves()).unwrap_or_default();
                let tile = |id: &&str| leaves.contains(id);
                let last = || before?.tiled_focus.get(&w.id).map(String::as_str).filter(tile);
                w.focus.as_deref().filter(tile).or_else(last).map(|id| (w.id.clone(), id.to_string()))
            })
            .collect();
    }
}

#[derive(Debug, Clone)]
pub struct Block {
    pub id: String,
    pub window_id: String,
    /// The label it was given (`leon toggle` finds a drawer by it).
    pub label: Option<String>,
    pub command: Vec<String>,
    /// The title the program set, and where it is (`session.list_blocks`).
    pub title: Option<String>,
    pub cwd: Option<String>,
    /// The program in front (`foreground`): None from a server older than that field, `Some(None)`
    /// when the server names none (the program exited, or another user's is in front).
    pub foreground: Option<Option<String>>,
    /// Program-status records, least recently reported first.
    pub status: Vec<Record>,
}

impl Block {
    /// The records most recently reported first, as `status::indicator_of` takes them.
    pub fn newest_first(&self) -> Vec<&Record> {
        self.status.iter().rev().collect()
    }
}

/// The server's sessions and the watched blocks' screens, as last received.
#[derive(Default)]
pub struct Model {
    /// Shared with the window's copy, which a commit only points at (no record is copied).
    pub sessions: Arc<Vec<Session>>,
    pub screens: HashMap<String, Arc<Frame>>,
    /// Per block, how many screen updates came: what a redraw check compares.
    pub screen_updates: HashMap<String, u64>,
    /// How many times the sessions were fetched (each `refresh`, on any thread).
    pub fetches: u64,
}

impl Model {
    pub fn session(&self, id: &str) -> Option<&Session> {
        self.sessions.iter().find(|s| s.id == id)
    }

    /// A block and the session it belongs to.
    pub fn block(&self, id: &str) -> Option<(&Session, &Block)> {
        self.sessions.iter().find_map(|s| s.blocks.iter().find(|b| b.id == id).map(|b| (s, b)))
    }
}
