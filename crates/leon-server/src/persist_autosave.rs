//! Saving while the server runs (persist.rs saves at the stop).

use super::*;

/// How long after a change to the model (a session, tab or pane made, closed, moved or renamed)
/// it is saved, at most, and how often the screens' text is checked for a change to save.
const AFTER_CHANGE: Duration = Duration::from_secs(1);
const TEXT_EVERY: Duration = Duration::from_secs(60);

/// Saving while the server runs, so a crash loses seconds instead of everything since the last
/// clean stop (whose save it would otherwise restore). The serve loop ticks it, so it stops before
/// the final save or discard, which is always the last write.
pub struct Autosave {
    /// Each session's id and revision when last looked at.
    seen: Vec<(String, u64)>,
    changed: Option<Instant>,
    checked: Instant,
    /// A fingerprint of the sessions last written (the time stamp left out).
    written: Option<u64>,
}

impl Default for Autosave {
    fn default() -> Self {
        // once soon after start too: a server started with --restore=false must not leave the old file
        Autosave { seen: Vec::new(), changed: Some(Instant::now()), checked: Instant::now(), written: None }
    }
}

impl Autosave {
    pub fn tick(&mut self, ctx: &Ctx, file: &Path) {
        let saved = {
            let st = ctx.state();
            let seen: Vec<_> = st.sessions.iter().map(|s| (s.id.clone(), s.revision)).collect();
            if seen != self.seen {
                self.seen = seen;
                // the first unsaved change counts: a model that keeps changing is still saved
                self.changed.get_or_insert_with(Instant::now);
            }
            let due = self.changed.is_some_and(|t| t.elapsed() >= AFTER_CHANGE) || self.checked.elapsed() >= TEXT_EVERY;
            if !due {
                return;
            }
            self.changed = None;
            self.checked = Instant::now();
            snapshot(&st) // under the lock; serialized and written after it
        };
        let Ok(sessions) = serde_json::to_vec(&saved.sessions) else { return };
        let mut h = std::collections::hash_map::DefaultHasher::new();
        sessions.hash(&mut h);
        let fingerprint = h.finish();
        if self.written == Some(fingerprint) {
            return;
        }
        match write(&saved, file) {
            Ok(()) => self.written = Some(fingerprint),
            Err(e) => eprintln!("leon: warning: can't save sessions to {}: {e}", file.display()),
        }
    }
}
