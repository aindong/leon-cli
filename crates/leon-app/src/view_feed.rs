//! Each machine's model as an entity (ADR R4, R8): the reader thread writes the remote's model,
//! wakes the window (a futures channel), and the wake task commits that model into the window's
//! copy on the main thread (`Remote::commit`). Each `HostModel` emits what moved (its sessions,
//! screens by block, a keymap change); the workspace subscribes and notifies only the views whose
//! drawing changed: a pushed screen its pane's grid, a refetch the sections whose input moved,
//! and the root only when what it draws itself moved (`RootInput`).

use std::rc::Rc;

use gpui::{Entity, EventEmitter, Subscription};

use super::*;

/// What moved in one machine's model.
pub(super) enum ModelEvent {
    Sessions,
    Screens(Vec<String>),
    Keymap,
}

/// One machine's model as the window shows it: `Remote::model` once committed here.
pub(super) struct HostModel {
    pub(super) remote: Rc<Remote>,
}

impl EventEmitter<ModelEvent> for HostModel {}

impl HostModel {
    fn commit(&mut self, cx: &mut Context<Self>) {
        let c = self.remote.commit();
        if c.sessions {
            cx.emit(ModelEvent::Sessions);
        }
        if !c.screens.is_empty() {
            cx.emit(ModelEvent::Screens(c.screens));
        }
        if c.keymap {
            cx.emit(ModelEvent::Keymap);
        }
    }
}

/// A machine's model entity and the workspace's subscription to it.
pub(super) struct ModelSlot {
    model: Entity<HostModel>,
    _events: Subscription,
}

/// What the root draws from the model and the machines, besides its children's inputs: the
/// sessions' tabs and layouts (each session's revision moves with layout, focus, zoom and
/// overlays), the machines' states, the selection; with an overlay up that lists the model (the
/// palette, the switcher, the overview), every fetch. Compared on each model change (R4's
/// stop-gap shape, as the sections' inputs are).
#[derive(PartialEq, Clone, Default)]
pub(super) struct RootInput {
    sessions: Vec<SessionTabs>,
    machines: Vec<(String, crate::hosts::HostState)>,
    selected: (usize, Option<String>),
    moving: bool,
    listing: Option<Vec<u64>>,
    stopped: bool,
}

/// A session as the root draws it: (machine, id, label, active tab, revision, each tab's id and label).
type SessionTabs = (usize, String, Option<String>, usize, u64, Vec<(String, Option<String>)>);

/// What a wake can bring that the root draws, from the machines' own threads: an update's phase
/// and the tailnet's reads.
pub(super) type WakeInput = (u64, u64);

impl Workspace {
    /// The wake task: the reader threads' and other threads' wakes, handled in order (one pending
    /// wake is enough: the handler takes all that came).
    pub(super) fn start_wakes(
        mut woken: futures::channel::mpsc::Receiver<()>,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Task<()> {
        cx.spawn_in(window, async move |this, cx| loop {
            if this.update_in(cx, |ws, window, cx| ws.on_wake(window, cx)).is_err() {
                return;
            }
            if woken.next().await.is_none() {
                return;
            }
        })
    }

    /// Give every connected machine a model entity, and drop those of machines gone.
    fn sync_models(&mut self, window: &Window, cx: &mut Context<Self>) {
        let remotes: Vec<Rc<Remote>> = self.hosts.remotes().map(|(_, r)| r.clone()).collect();
        self.models.retain(|(r, _)| remotes.iter().any(|x| Rc::ptr_eq(x, r)));
        for r in remotes {
            if self.models.iter().any(|(m, _)| Rc::ptr_eq(m, &r)) {
                continue;
            }
            let model = cx.new(|_| HostModel { remote: r.clone() });
            let events = cx.subscribe_in(&model, window, |ws, m, ev, window, cx| {
                let remote = m.read(cx).remote.clone();
                ws.on_model(&remote, ev, window, cx)
            });
            self.models.push((r, ModelSlot { model, _events: events }));
        }
    }

    /// Commit every machine's model into the window's copy: the events follow.
    pub(super) fn commit_models(&mut self, window: &Window, cx: &mut Context<Self>) {
        self.sync_models(window, cx);
        for (_, slot) in &self.models {
            slot.model.update(cx, |m, cx| m.commit(cx));
        }
    }

    /// Woken: the hosts' attempts, the models, the replies to calls, and what other threads left
    /// (an update's phase, a notification's click, the keymap's reply). Nothing timed: each timed
    /// thing has a task of its own.
    pub(super) fn on_wake(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.wakes.0 += 1;
        if !self.hosts.local().alive() {
            // the menu's end-all quits; a stop from outside is said, not a vanishing window
            if self.ending {
                cx.quit();
            } else if !self.stopped {
                self.stopped = true;
                cx.notify();
            }
            return;
        }
        let hosts = self.poll_hosts();
        self.arm_host_retry(window, cx);
        self.commit_models(window, cx);
        let made = self.select_made();
        let replied = self.poll_terms();
        self.card_poll(cx);
        if let Some(text) = self.term.clip.take() {
            cx.write_to_clipboard(gpui::ClipboardItem::new_string(text));
        }
        self.take_clicks(cx);
        self.refresh_keymap(cx);
        self.stats.borrow_mut().report(); // the last frames before a quiet spell
        let seen = (self.updates.generation(), self.hosts.tailnet_reads());
        if replied || hosts || made || seen != self.wake_seen {
            self.wake_seen = seen;
            self.root_changed(cx);
        }
    }

    /// What moved in machine `remote`'s model.
    fn on_model(&mut self, remote: &Rc<Remote>, ev: &ModelEvent, window: &mut Window, cx: &mut Context<Self>) {
        match ev {
            ModelEvent::Keymap if Rc::ptr_eq(remote, self.hosts.local()) => {
                self.keymap_asked = None; // asked again now (the keymap is This Mac's)
                self.refresh_keymap(cx);
            }
            ModelEvent::Keymap => {}
            ModelEvent::Sessions => self.sessions_moved(window, cx),
            ModelEvent::Screens(ids) => {
                if Rc::ptr_eq(remote, &self.app) {
                    self.screens_changed(ids, window, cx);
                }
                self.refresh_thumbs(true, cx);
                self.arm_thumbs(window, cx);
            }
        }
    }

    /// A machine's sessions moved (its model event; tests call it directly): attention and the
    /// thumbnails look again, what render draws from is brought up to date, the grids and
    /// sections given their inputs, and the root notified only when what it draws moved.
    pub fn sessions_moved(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.poll_attention(window, cx);
        self.refresh_thumbs(true, cx);
        self.arm_thumbs(window, cx);
        self.prepare();
        if self.root_input() != self.root_seen {
            self.root_changed(cx); // the observer gives the views their inputs
        } else {
            self.sync_views(window, cx);
        }
    }

    /// The root draws something new: its observer brings every view's input up to date (`prepare`).
    pub(super) fn root_changed(&mut self, cx: &mut Context<Self>) {
        cx.notify();
    }

    /// Pushed screens: each shown pane's grid brought up to date alone; the root only for a pane
    /// scrolled back, whose "↓ N new lines" it counts. A find on one of them looks again.
    fn screens_changed(&mut self, ids: &[String], window: &mut Window, cx: &mut Context<Self>) {
        let mut scrolled = false;
        for id in ids.iter().filter(|id| self.grid_fits(id)).cloned().collect::<Vec<_>>() {
            self.sync_pane(&id);
            self.sync_grid(&id, cx);
            scrolled |= self.view_top(&id).is_some();
        }
        if self.term.find.as_ref().is_some_and(|f| ids.contains(&f.block)) {
            self.find_output(window, cx);
        }
        self.feed_cursor(window);
        if scrolled {
            self.root_changed(cx);
        }
    }

    /// What the root draws from the model now.
    pub(super) fn root_input(&self) -> RootInput {
        let mut sessions = Vec::new();
        for (host, remote) in self.hosts.remotes() {
            for s in remote.model().sessions.iter() {
                let tabs = s.windows.iter().map(|w| (w.id.clone(), w.label.clone())).collect();
                sessions.push((host, s.id.clone(), s.label.clone(), self.active_of(s), s.revision, tabs));
            }
        }
        let listing = (self.palette.is_some() || self.overview_shown() || self.row_menu().is_some())
            .then(|| self.hosts.remotes().map(|(_, r)| r.model().fetches).collect());
        RootInput {
            sessions,
            machines: self.hosts.machines().iter().map(|m| (m.label.clone(), m.state())).collect(),
            selected: (self.host, self.session_id.clone()),
            moving: self.animating(),
            listing,
            stopped: self.stopped,
        }
    }

    /// How many times the wake handler ran (tests: it runs only when woken).
    pub fn wakes(&self) -> u64 {
        self.wakes.0
    }

    /// How many times the workspace was notified: told the root draws something new (tests;
    /// GPUI renders the root on every frame it draws, so `renders` counts frames).
    pub fn root_redraws(&self) -> u64 {
        self.wakes.1
    }
}

/// The window's timed things, each a task of its own on GPUI's executor (R6, R8): it runs once at
/// its time and is replaced when the time moves.
#[derive(Default)]
pub(super) struct Deadlines {
    notices: Option<(Instant, Task<()>)>,
    retry: Option<(Instant, Task<()>)>,
    find: Option<(Instant, Task<()>)>,
    thumbs: Option<(Instant, Task<()>)>,
    pub(super) activity: Option<Task<()>>,
}

/// Arm `slot` for `at` with the task `task` makes, unless it already is; None disarms it.
fn arm(slot: &mut Option<(Instant, Task<()>)>, at: Option<Instant>, task: impl FnOnce(Instant) -> Task<()>) {
    match at {
        None => *slot = None,
        Some(at) if slot.as_ref().is_some_and(|(t, _)| *t == at) => {}
        Some(at) => *slot = Some((at, task(at))),
    }
}

impl Workspace {
    /// A task that runs `f` with the window after `wait`.
    fn after(
        &self,
        wait: Duration,
        window: &Window,
        cx: &mut Context<Self>,
        f: impl FnOnce(&mut Self, &mut Window, &mut Context<Self>) + 'static,
    ) -> Task<()> {
        cx.spawn_in(window, async move |this, cx| {
            cx.background_executor().timer(wait).await;
            this.update_in(cx, |ws, window, cx| f(ws, window, cx)).ok();
        })
    }

    /// The summary of what a storm of notices held back, once there is room (GPUI's clock).
    pub(super) fn arm_notices(&mut self, window: &Window, cx: &mut Context<Self>) {
        let (due, now) = (self.notifier.due(), self.clock_now());
        let mut slot = self.deadlines.notices.take();
        arm(&mut slot, due, |at| {
            self.after(at.saturating_duration_since(now), window, cx, |ws, window, cx| {
                ws.deadlines.notices = None;
                ws.flush_notices(window, cx);
            })
        });
        self.deadlines.notices = slot;
    }

    /// A failed host's next attempt (the hosts list times its backoff on the real clock).
    fn arm_host_retry(&mut self, window: &Window, cx: &mut Context<Self>) {
        let (due, now) = (self.hosts.next_retry(), Instant::now());
        let mut slot = self.deadlines.retry.take();
        arm(&mut slot, due, |at| {
            self.after(at.saturating_duration_since(now), window, cx, |ws, _, _| {
                ws.deadlines.retry = None;
                (ws.wake)();
            })
        });
        self.deadlines.retry = slot;
    }

    /// Output came in a pane a find is open on: look again now if the last look is old enough,
    /// else once it is.
    fn find_output(&mut self, window: &Window, cx: &mut Context<Self>) {
        if self.find_due() {
            self.deadlines.find = None;
            self.run_find();
            return;
        }
        let due = self.find_rerun_in().map(|d| Instant::now() + d);
        let mut slot = self.deadlines.find.take();
        arm(&mut slot, due, |at| {
            self.after(at.saturating_duration_since(Instant::now()), window, cx, |ws, window, cx| {
                ws.deadlines.find = None;
                ws.find_output(window, cx);
            })
        });
        self.deadlines.find = slot;
    }

    /// A thumbnail's refresh `THUMB_EVERY` held back.
    pub(super) fn arm_thumbs(&mut self, window: &Window, cx: &mut Context<Self>) {
        let due = self.thumbs_held();
        let mut slot = self.deadlines.thumbs.take();
        arm(&mut slot, due, |at| {
            self.after(at.saturating_duration_since(Instant::now()), window, cx, |ws, window, cx| {
                ws.deadlines.thumbs = None;
                ws.refresh_thumbs(true, cx);
                ws.arm_thumbs(window, cx);
            })
        });
        self.deadlines.thumbs = slot;
    }

    /// The Activity list's waited times (`12s`) turn once a second while it lists something and
    /// frames are drawn: the sidebar's input moves, nothing else. A window drawing no frames
    /// (hidden, covered) stops it; the next frame starts it again (`drew_frame`).
    pub(super) fn tick_activity(&mut self, window: &Window, cx: &mut Context<Self>) {
        let wanted = self.activity && !self.attention.rows().is_empty();
        if !wanted {
            self.deadlines.activity = None;
            return;
        }
        if self.deadlines.activity.is_some() {
            return;
        }
        let frames = self.frames;
        self.deadlines.activity = Some(cx.spawn_in(window, async move |this, cx| {
            let mut seen = frames;
            loop {
                cx.background_executor().timer(WAITED_TICK).await;
                let on = this.update_in(cx, |ws, window, cx| {
                    let drawn = ws.frames != seen;
                    seen = ws.frames;
                    if drawn && ws.activity && !ws.attention.rows().is_empty() {
                        ws.sync_sections(window, cx);
                        return true;
                    }
                    ws.deadlines.activity = None;
                    false
                });
                if !matches!(on, Ok(true)) {
                    return;
                }
            }
        }));
    }

    /// The daily update look, asked every `LOOK_EVERY` (`Updater::daily` decides whether a day
    /// went by), and the frame stats' report each second while they are on.
    pub(super) fn start_clocks(window: &Window, cx: &mut Context<Self>) -> (Task<()>, Option<Task<()>>) {
        let looks = cx.spawn_in(window, async move |this, cx| loop {
            cx.background_executor().timer(update_view::LOOK_EVERY).await;
            if this.update(cx, |ws, _| ws.look_for_updates()).is_err() {
                return;
            }
        });
        let stats = (std::env::var("LEON_FRAME_STATS").as_deref() == Ok("1")).then(|| {
            cx.spawn_in(window, async move |this, cx| loop {
                cx.background_executor().timer(Duration::from_secs(1)).await;
                if this.update(cx, |ws, _| ws.stats.borrow_mut().report()).is_err() {
                    return;
                }
            })
        });
        (looks, stats)
    }
}

/// How long the Activity list's waited times (`12s`) stay right.
const WAITED_TICK: Duration = Duration::from_secs(1);
