//! The keymap and settings fetched from This Mac's server, and the bookkeeping after each frame.
//! What a wake brings is view_feed.rs.

use super::*;

/// The last `client.keymap` reply, handed over from the runtime thread that fetched it.
#[derive(Default)]
pub(super) struct KeymapInbox {
    pub(super) asking: bool,
    reply: Option<Value>,
    /// The `server.settings` reply asked for when the keymap's generation moved to the one given.
    settings: Option<(u64, Value)>,
}

impl Workspace {
    /// After a frame was drawn (render defers it): the pane it showed noted as seen, and the
    /// clocks that pause while no frame is drawn (the Pulse, the Activity list's) go on, as after
    /// a hidden spell; the cursor shader overlay follows the frame (a slide, a tab switch).
    pub(super) fn drew_frame(&mut self, showed: Shown, window: &mut Window, cx: &mut Context<Self>) {
        self.frames += 1;
        self.note_focus(showed);
        let wanted = self.pulse_wanted;
        self.pulse.update(cx, |p, cx| p.drew(wanted, cx));
        self.tick_activity(window, cx);
        self.feed_cursor(window);
    }
}

/// The keymap and settings fetched from the server.
impl Workspace {
    /// Ask the server for its keymap, off the UI thread (a reload holds the config for up to 5 s):
    /// the first time, and after its `keymap_changed` event (`keymap_asked` cleared). Apply a
    /// reply whose generation moved, and then the settings of that reload (every reload moves the
    /// generation, and `settings_changed` only comes with one).
    pub(super) fn refresh_keymap(&mut self, cx: &mut Context<Self>) {
        let settings = lock(&self.inbox).settings.take();
        // one asked for under an older generation may come after a newer one: it is dropped
        if let Some((_, settings)) = settings.filter(|(g, _)| *g == self.keymap.generation) {
            self.apply_settings(&settings);
            self.glass_later(cx);
            cx.notify();
        }
        let reply = {
            let mut inbox = lock(&self.inbox);
            let due = self.keymap_asked.is_none();
            if due && !inbox.asking {
                inbox.asking = true;
                self.keymap_asked = Some(Instant::now());
                let (back, wake) = (self.inbox.clone(), self.wake.clone());
                self.hosts.local().spawn("client.keymap", json!({}), move |r| {
                    let mut inbox = lock(&back);
                    inbox.asking = false;
                    inbox.reply = r.ok();
                    drop(inbox);
                    wake(); // and a `keymap_changed` that came while this was out asks again then
                });
            }
            inbox.reply.take()
        };
        if let Some(km) = reply.filter(|km| km["generation"].as_u64() != Some(self.keymap.generation)) {
            self.use_keymap(&km);
            let (back, generation, wake) = (self.inbox.clone(), self.keymap.generation, self.wake.clone());
            self.hosts.local().spawn("server.settings", json!({}), move |r| {
                let mut inbox = lock(&back);
                // a slow reply asked for under an older generation never replaces a newer one
                if let (Ok(s), true) = (r, inbox.settings.as_ref().is_none_or(|(g, _)| *g <= generation)) {
                    inbox.settings = Some((generation, s));
                    drop(inbox);
                    wake();
                }
            });
            cx.notify();
        }
    }

    fn use_keymap(&mut self, km: &Value) {
        self.keymap = Keymap::app(km);
        // a relative path is the server's, from its own working directory: not one to use here
        let path = km["path"].as_str().map(PathBuf::from).filter(|p| p.is_absolute());
        self.config_path = path.or_else(leon_lua::config_path);
    }

    /// Fetch the keymap and the settings and apply them now, waiting for the server (tests, and
    /// nothing on a hot path); what they change redraws.
    pub fn sync_keymap(&mut self, cx: &mut Context<Self>) {
        if let Ok(km) = self.hosts.local().call("client.keymap", json!({})) {
            self.use_keymap(&km);
        }
        if let Ok(settings) = self.hosts.local().call("server.settings", json!({})) {
            self.apply_settings(&settings);
        }
        cx.notify();
    }
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}
