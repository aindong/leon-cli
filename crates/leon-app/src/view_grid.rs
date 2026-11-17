//! Each shown pane's grid as a view of its own, shown cached beside its header (ADR R3): the
//! workspace keeps the panes' state and gives each grid what it draws, notifying the grid only
//! when that changed (R4), so a busy pane renders its grid and a quiet pane's is reused.

use std::collections::HashMap;

use super::*;

/// Everything a grid draws: a grid is notified when, and only when, this changes.
#[derive(Clone, PartialEq)]
pub(super) struct GridInput {
    /// The view, faded with its pane, with copy mode's badge or the hint labels stamped on it.
    frame: Frame,
    marks: Vec<CellMark>,
    /// Its cursor is the one taking keys: the pane focused in the key window.
    active: bool,
    family: SharedString,
    font_size: f32,
    theme: Theme,
    /// It fits its PTY to its bounds; not a closing pane's (its block is gone) while it shrinks.
    fits: bool,
}

/// What a grid's last render drew (tests).
#[derive(Clone, Debug)]
pub struct GridDrawn {
    pub text: String,
    pub marks: usize,
    pub active: bool,
    pub font_size: f32,
    pub theme: Theme,
}

/// A shown pane's grid view, what it was last given (when it had rendered `given` times), the
/// pushed screen that was from, and the session and machine it fits the PTY on.
pub(super) struct GridSlot {
    view: gpui::Entity<PaneGrid>,
    input: GridInput,
    given: u64,
    screen: Option<u64>,
    sid: String,
    app: Rc<Remote>,
}

/// Each pane's pushed-screen count, read before what it is drawn from: a push that lands after
/// the read is one the poll still sees (`poll_grids`).
pub(super) type Screens = HashMap<String, u64>;

/// A pane's grid: one canvas that fits the PTY in prepaint and paints the frame (view_paint.rs).
pub(super) struct PaneGrid {
    input: GridInput,
    app: Rc<Remote>,
    sid: String,
    block: String,
    stats: Rc<RefCell<Stats>>,
    painted: scroll::PaintedMap,
    renders: Rc<RefCell<HashMap<String, u64>>>,
}

impl Render for PaneGrid {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        drawing_cached(cx);
        *self.renders.borrow_mut().entry(self.block.clone()).or_default() += 1;
        canvas_of(&self.input, self.app.clone(), (self.sid.clone(), self.block.clone()), (&self.stats, &self.painted))
    }
}

fn canvas_of(
    i: &GridInput,
    app: Rc<Remote>,
    ids: (String, String),
    (stats, painted): (&Rc<RefCell<Stats>>, &scroll::PaintedMap),
) -> impl IntoElement {
    terminal_canvas(
        app,
        ids,
        (i.frame.clone(), i.marks.clone()),
        i.active,
        (i.family.clone(), i.font_size, i.theme),
        (stats.clone(), painted.clone()),
        i.fits,
    )
}

impl Workspace {
    /// What pane `id`'s grid draws now.
    fn grid_input(&self, id: &str) -> GridInput {
        let theme = self.pane_theme(id);
        let focused = self.target().is_some_and(|(_, t, _)| t == id);
        let view = self.drawn(id);
        let marks = self.pane_marks(id, &view);
        GridInput {
            frame: crate::fade::frame(view.frame, theme.chrome, self.pane_fade(id)),
            marks,
            active: self.window_active && focused,
            family: self.font_family.clone(),
            font_size: self.font_size,
            theme,
            fits: true,
        }
    }

    /// The panes whose grids are drawn: the active window's, unless the overview or a stopped
    /// server's notice takes their place.
    fn grid_panes(&self) -> Panes {
        if self.stopped || self.overview_shown() {
            return Vec::new();
        }
        self.shown().map(|(_, ids)| ids).unwrap_or_default()
    }

    /// Give every shown pane's grid what it draws now, notifying the ones that changed; make the
    /// grids of panes that came on screen and drop those of panes that left. After `prepare`, on
    /// every notify of the workspace; it notifies only grids, never the workspace.
    pub(super) fn sync_grids(&mut self, seen: &Screens, cx: &mut Context<Self>) {
        let shown = self.grid_panes();
        // a closing pane's grid, and those a zoom covers, stay as they were until their motion ends
        let ghosts = self.layout_ghosts().to_vec();
        self.grids.retain(|id, _| shown.contains(id) || ghosts.contains(id));
        self.grid_renders.borrow_mut().retain(|id, _| shown.contains(id) || ghosts.contains(id));
        for id in &shown {
            self.sync_grid(id, seen.get(id).copied(), cx);
        }
        for id in &ghosts {
            let Some(slot) = self.grids.get_mut(id).filter(|s| s.input.fits) else { continue };
            slot.input.fits = false;
            let input = slot.input.clone();
            slot.view.update(cx, |g, cx| {
                g.input = input;
                cx.notify();
            });
        }
    }

    /// The pushed-screen counts now, read before `sync_pane` and the grids' inputs.
    pub(super) fn screens_seen(&self) -> Screens {
        self.app.model().screen_updates.clone()
    }

    fn sync_grid(&mut self, id: &str, screen: Option<u64>, cx: &mut Context<Self>) {
        let input = self.grid_input(id);
        let sid = self.session_id.clone().unwrap_or_default();
        // a pane gone to another session or machine fits its PTY there: a new grid
        if self.grids.get(id).is_some_and(|s| s.sid != sid || !Rc::ptr_eq(&s.app, &self.app)) {
            self.grids.remove(id);
        }
        match self.grids.get_mut(id) {
            Some(slot) => {
                slot.screen = screen;
                if slot.input != input {
                    slot.input = input.clone();
                    slot.given = self.grid_renders.borrow().get(id).copied().unwrap_or(0);
                    slot.view.update(cx, |g, cx| {
                        g.input = input;
                        cx.notify();
                    });
                }
            }
            None => {
                let grid = PaneGrid {
                    input: input.clone(),
                    app: self.app.clone(),
                    sid: sid.clone(),
                    block: id.to_string(),
                    stats: self.stats.clone(),
                    painted: self.term.painted.clone(),
                    renders: self.grid_renders.clone(),
                };
                let view = cx.new(|_| grid);
                let app = self.app.clone();
                self.grids.insert(id.to_string(), GridSlot { view, input, given: 0, screen, sid, app });
            }
        }
    }

    /// From the poll: the grids of panes whose pushed screen moved, brought up to date (the
    /// screen is the workspace's no longer: `signature` leaves it out). Returns whether one of
    /// them is scrolled back, whose new lines the root's pill counts.
    pub(super) fn poll_grids(&mut self, cx: &mut Context<Self>) -> bool {
        let now = self.screens_seen();
        // another machine's grids, until the notify after a host switch drops them
        let here = |s: &GridSlot| Rc::ptr_eq(&s.app, &self.app);
        let moved: Vec<String> = self
            .grids
            .iter()
            .filter(|(id, s)| here(s) && s.screen != now.get(*id).copied() && s.input.fits)
            .map(|(id, _)| id.clone())
            .collect();
        let mut scrolled = false;
        for id in moved {
            self.sync_pane(&id);
            self.sync_grid(&id, now.get(&id).copied(), cx);
            scrolled |= self.view_top(&id).is_some();
        }
        scrolled
    }

    /// Pane `id`'s grid, shown cached; the plain canvas when it has none yet (a frame before any
    /// notify, or a pane the model shows before the poll ran): never a view made while drawing.
    pub(super) fn grid_element(&self, id: &str) -> gpui::AnyElement {
        let grid = match self.grids.get(id) {
            Some(s) => gpui::AnyView::from(s.view.clone())
                .cached(gpui::StyleRefinement::default().size_full())
                .into_any_element(),
            None => {
                let ids = (self.session_id.clone().unwrap_or_default(), id.to_string());
                canvas_of(&self.grid_input(id), self.app.clone(), ids, (&self.stats, &self.term.painted))
                    .into_any_element()
            }
        };
        // while panes move it keeps the size its PTY has, clipped by its moving tile (one resize, at the end)
        match self.layout_held(id) {
            Some(h) => div().flex_none().w(px(h.w)).h(px(h.h)).child(grid).into_any_element(),
            None => grid,
        }
    }

    /// Per shown pane, how often its grid rendered.
    pub fn grid_renders(&self) -> HashMap<String, u64> {
        self.grid_renders.borrow().clone()
    }

    /// What pane `id`'s grid drew last: its input, once it rendered after being given it.
    pub fn grid_drew(&self, id: &str) -> Option<GridDrawn> {
        let slot = self.grids.get(id)?;
        let renders = self.grid_renders.borrow().get(id).copied().unwrap_or(0);
        let i = (renders > slot.given).then_some(&slot.input)?;
        Some(GridDrawn {
            text: i.frame.text(),
            marks: i.marks.len(),
            active: i.active,
            font_size: i.font_size,
            theme: i.theme,
        })
    }
}
