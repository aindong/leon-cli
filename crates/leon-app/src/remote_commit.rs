//! The window's copy of a remote's model (GPUI phase 5, docs/architecture/gpui.md R4): the reader
//! thread writes the remote's own model, and the window's thread commits it into the copy it draws
//! from, so what it draws changes only there. `leon attach` never commits and reads the received
//! model.

use super::*;

/// What a `Remote::commit` brought into the window's copy of the model.
#[derive(Debug, Default)]
pub struct Commit {
    /// The sessions were fetched again.
    pub sessions: bool,
    /// The blocks whose screen came or went.
    pub screens: Vec<String>,
    /// The server's keymap or settings may have changed (`keymap_changed`, `settings_changed`).
    pub keymap: bool,
}

impl Remote {
    /// The model as last received, whatever the window committed (tests).
    pub fn live(&self) -> MutexGuard<'_, Model> {
        lock(&self.shared.model)
    }

    /// Bring the window's copy of the model up to what the reader received, on the window's
    /// thread only, and say what moved. The first commit starts the copy.
    pub fn commit(&self) -> Commit {
        let shown = self.shown.get_or_init(Mutex::default);
        let live = lock(&self.shared.model);
        let mut m = lock(shown);
        let mut c = Commit { keymap: self.shared.keymap_news.swap(false, Ordering::SeqCst), ..Commit::default() };
        if live.fetches != m.fetches {
            (m.sessions, m.fetches, c.sessions) = (live.sessions.clone(), live.fetches, true);
        }
        // screens dropped (unwatched, a re-attach) go from the copy too
        let gone: Vec<String> = m.screens.keys().filter(|id| !live.screens.contains_key(*id)).cloned().collect();
        for id in gone {
            m.screens.remove(&id);
            c.screens.push(id);
        }
        m.screen_updates.retain(|id, _| live.screen_updates.contains_key(id));
        for (id, n) in &live.screen_updates {
            if m.screen_updates.get(id) == Some(n) {
                continue;
            }
            m.screen_updates.insert(id.clone(), *n);
            if let Some(f) = live.screens.get(id) {
                m.screens.insert(id.clone(), f.clone());
            }
            if !c.screens.contains(id) {
                c.screens.push(id.clone());
            }
        }
        c
    }
}
