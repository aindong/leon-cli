//! The poll: what the workspace does when woken (the reader thread's notice, a reply, a host's
//! attempt, an update's phase, a notification's click) or when something timed is due. With
//! nothing changing it does not run at all (docs/performance.md, idle CPU).

use super::*;

/// The last `client.keymap` reply, handed over from the runtime thread that fetched it.
#[derive(Default)]
pub(super) struct KeymapInbox {
    pub(super) asking: bool,
    reply: Option<Value>,
    /// The `server.settings` reply asked for when the keymap's generation moved to the one given.
    settings: Option<(u64, Value)>,
}

/// How long the Activity list's waited times (`12s`) stay right.
const WAITED_TICK: Duration = Duration::from_secs(1);

impl Workspace {
    /// The poll, on `woken` and its own deadlines, and the 1 s tick beside it.
    pub(super) fn start_poll(
        mut woken: futures::channel::mpsc::Receiver<()>,
        cx: &mut Context<Self>,
    ) -> (Task<()>, Task<()>) {
        // the poll runs when woken, or when something timed is due (`next_poll`); never on a clock
        let poll = cx.spawn(async move |this, cx| {
            let mut wait = Some(Duration::ZERO);
            loop {
                match wait {
                    Some(d) => drop(futures::future::select(cx.background_executor().timer(d), woken.next()).await),
                    None => drop(woken.next().await),
                }
                let Ok(next) = this.update(cx, |ws, cx| ws.poll(cx)) else { return };
                wait = next;
            }
        });
        // the keymap's ask (a reload elsewhere shows up this late), the daily update look and the
        // frame stats keep a clock of their own; what they bring wakes the poll
        let tick = cx.spawn(async move |this, cx| loop {
            cx.background_executor().timer(KEYMAP_POLL).await;
            let alive = this.update(cx, |ws, cx| {
                ws.refresh_keymap(true, cx);
                ws.look_for_updates();
                ws.stats.borrow_mut().report();
            });
            if alive.is_err() {
                return;
            }
        });
        (poll, tick)
    }

    /// One look: take what came, post what is news, redraw when something drawn moved. Returns
    /// when to look again without a wake.
    pub(super) fn poll(&mut self, cx: &mut Context<Self>) -> Option<Duration> {
        self.polls.0 += 1;
        if !self.hosts.local().alive() {
            // the menu's end-all quits; a stop from outside is said, not a vanishing window
            if self.ending {
                cx.quit();
            } else if !self.stopped {
                self.stopped = true;
                cx.notify();
            }
            self.planned = None;
            return None;
        }
        if let Some((app, event)) = self.focus_event.take() {
            app.send_then("block.event", event, drop); // a pane already gone is fine
        }
        self.poll_hosts();
        let replied = self.poll_terms();
        self.card_found(cx);
        // a pushed screen redraws its pane's grid alone; the root only for a pane scrolled back,
        // whose "↓ N new lines" it counts
        let scrolled = self.poll_grids(cx);
        if let Some(text) = self.term.clip.take() {
            cx.write_to_clipboard(gpui::ClipboardItem::new_string(text));
        }
        self.poll_attention(cx);
        self.refresh_thumbs(true, cx);
        self.stats.borrow_mut().report(); // and on the tick, for the last frames before a quiet spell
        self.refresh_keymap(false, cx);
        let now = self.signature();
        if now != self.seen || replied || scrolled {
            self.seen = now;
            self.polls.1 += 1;
            self.undrawn = self.undrawn.saturating_add(1);
            cx.notify();
        }
        let next = self.next_poll();
        self.planned = next.map(|d| Instant::now() + d);
        next
    }

    /// The soonest timed thing pending, never sooner than `POLL`; None when only a wake can bring
    /// news (the spinner has its own clock, view_pulse.rs; slides and fades step on the frames
    /// render asks GPUI for, `ask_frame`). The waited times and a find's re-run only matter on
    /// screen: once a second redraw asked for is not drawn (hidden, covered), they wait for the
    /// next frame, which wakes the poll again (`drew_frame`).
    pub(super) fn next_poll(&self) -> Option<Duration> {
        let now = Instant::now();
        let mut due = Vec::new();
        if self.undrawn < 2 {
            if self.activity && !self.attention.rows().is_empty() {
                due.push(WAITED_TICK);
            }
            due.extend(self.find_rerun_in());
            due.extend(self.thumbs_due()); // a thumbnail's refresh `THUMB_EVERY` held back
        }
        due.extend(self.notifier.due().map(|t| t.saturating_duration_since(now)));
        due.extend(self.hosts.next_retry().map(|t| t.saturating_duration_since(now)));
        due.into_iter().min().map(|d| d.max(POLL))
    }

    /// After a frame was drawn (render defers it): the poll looks again at once if the state that
    /// frame drew has something timed due sooner than it planned for (the Activity list, a find's
    /// re-run), and the Pulse goes on (it pauses while no frame is drawn), as after a hidden spell.
    pub(super) fn drew_frame(&mut self, showed: Shown, cx: &mut Context<Self>) {
        self.undrawn = 0;
        self.note_focus(showed);
        let wanted = self.pulse_wanted;
        self.pulse.update(cx, |p, cx| p.drew(wanted, cx));
        let Some(d) = self.next_poll() else { return };
        if self.planned.is_none_or(|p| Instant::now() + d + POLL < p) {
            (self.wake)();
        }
    }
}

/// The keymap and settings the poll and the tick fetch from the server.
impl Workspace {
    /// Ask the server for its keymap, off the UI thread (a reload holds the config for up to 5 s):
    /// on the 1 s `tick`, else only for the first time or after a reload. Apply a reply whose
    /// generation moved, and then the settings of that reload (every reload moves the generation,
    /// and `settings_changed` only comes with one).
    pub(super) fn refresh_keymap(&mut self, tick: bool, cx: &mut Context<Self>) {
        let settings = lock(&self.inbox).settings.take();
        // one asked for under an older generation may come after a newer one: it is dropped
        if let Some((_, settings)) = settings.filter(|(g, _)| *g == self.keymap.generation) {
            self.apply_settings(&settings);
            self.glass_later(cx);
            cx.notify();
        }
        let reply = {
            let mut inbox = lock(&self.inbox);
            let due = tick || self.keymap_asked.is_none();
            if due && !inbox.asking {
                inbox.asking = true;
                self.keymap_asked = Some(Instant::now());
                let (back, wake, known) = (self.inbox.clone(), self.wake.clone(), self.keymap.generation);
                self.hosts.local().spawn("client.keymap", json!({}), move |r| {
                    let news = r.as_ref().is_ok_and(|km| km["generation"].as_u64() != Some(known));
                    let mut inbox = lock(&back);
                    inbox.asking = false;
                    inbox.reply = r.ok();
                    drop(inbox);
                    if news {
                        wake();
                    }
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
