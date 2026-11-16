//! The poll: what the workspace does when woken (the reader thread's notice, a reply, a host's
//! attempt, an update's phase, a notification's click) or when something timed is due. With
//! nothing changing it does not run at all (docs/performance.md, idle CPU).

use super::*;

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
                ws.refresh_keymap(cx);
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
            return None;
        }
        self.poll_hosts();
        let replied = self.poll_terms();
        if let Some(text) = self.term.clip.take() {
            cx.write_to_clipboard(gpui::ClipboardItem::new_string(text));
        }
        self.poll_attention(cx);
        self.refresh_keymap(cx);
        let now = self.signature();
        if now != self.seen || self.sliding() || self.fading() || replied {
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
    /// news. Slides, fades, the spinner, the waited times and a find's re-run only matter on
    /// screen: once a second redraw asked for is not drawn (hidden, covered), they wait for the
    /// next frame, whose render wakes the poll again.
    pub(super) fn next_poll(&self) -> Option<Duration> {
        let now = Instant::now();
        let mut due = Vec::new();
        if self.undrawn < 2 {
            if self.sliding() || self.fading() {
                due.push(POLL);
            }
            if self.spinning(&self.rows()).is_some() {
                let spin = SPIN.as_millis();
                due.push(Duration::from_millis((spin - self.epoch.elapsed().as_millis() % spin) as u64));
            }
            if self.activity && !self.attention.rows().is_empty() {
                due.push(WAITED_TICK);
            }
            due.extend(self.find_rerun_in());
        }
        due.extend(self.notifier.due().map(|t| t.saturating_duration_since(now)));
        due.extend(self.hosts.next_retry().map(|t| t.saturating_duration_since(now)));
        due.into_iter().min().map(|d| d.max(POLL))
    }

    /// From render: a frame was drawn, and the poll looks again at once if this frame started
    /// something timed sooner than it planned for (a slide, a fade, the spinner, the Activity list).
    pub(super) fn drew_frame(&mut self) {
        self.undrawn = 0;
        let Some(d) = self.next_poll() else { return };
        if self.planned.is_none_or(|p| Instant::now() + d + POLL < p) {
            (self.wake)();
        }
    }
}
