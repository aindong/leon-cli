//! The fade of the panes without the focus (`crate::fade`) in the workspace: before each frame
//! it heads every shown pane for its fade, and a pane is drawn with the window's theme faded
//! that far.

use std::collections::HashMap;

use super::*;
use crate::fade::{self, Fades};

pub(super) struct FadeUi {
    fades: Fades,
    /// GPUI's clock, which every motion reads (slides, fades, the theme's change, the overview):
    /// on the test platform it is the test clock (ADR R6, R10).
    clock: gpui::BackgroundExecutor,
    /// Whether the last render asked GPUI for an animation frame (tests can't see the request).
    asked: bool,
    /// Motion held at this time, as a test set it, whatever GPUI's clock says (a test that moves
    /// the clock for something else, such as a card's refresh, while a motion stands still).
    held: Option<Instant>,
    /// Each shown pane's status as the user last saw it, focused in the key window: a finish
    /// they watched is seen, though the server keeps it until they come back (`note_focus`).
    seen: HashMap<String, Option<Indicator>>,
}

impl FadeUi {
    pub(super) fn new(clock: gpui::BackgroundExecutor) -> Self {
        FadeUi { fades: Fades::default(), clock, asked: false, held: None, seen: HashMap::new() }
    }
}

impl Workspace {
    /// Hold every motion at `at`, whatever GPUI's clock says (tests).
    pub fn set_fade_clock(&mut self, at: Instant) {
        self.fade.held = Some(at);
    }

    /// How far pane `id` is faded in the frame last drawn: 0 at full strength, at most the
    /// `fade_inactive_panes` setting, or a theme change's dip while one fades in.
    pub fn pane_fade(&self, id: &str) -> f32 {
        self.fade.fades.amount(id).max(self.theme_dip())
    }

    /// The motions' clock: GPUI's, which the test platform fakes, unless a test held it.
    pub(super) fn motion_now(&self) -> Instant {
        self.fade.held.unwrap_or_else(|| self.fade.clock.now())
    }

    /// Whether anything moves: a drawer's slide, a fade, the theme's change, the overview's open
    /// or close. Render asks GPUI for the next frame while it holds (ADR R6).
    pub fn animating(&self) -> bool {
        self.sliding() || self.fading() || self.theme_shifting() || self.overview_moving() || self.layout_moving()
    }

    /// Ask for the next animation frame while anything moves, noting that it did beside the call
    /// (render): GPUI's test window drops the request, so tests read the note.
    pub(super) fn ask_frame(&mut self, window: &mut Window) {
        self.fade.asked = false;
        if self.animating() {
            window.request_animation_frame();
            self.fade.asked = true;
        }
    }

    /// Whether the last render asked GPUI for an animation frame (tests).
    pub fn frame_asked(&self) -> bool {
        self.fade.asked
    }

    /// The theme pane `id` is drawn with: the window's, faded as far as the pane is.
    pub fn pane_theme(&self, id: &str) -> Theme {
        fade::theme(&self.theme, self.pane_fade(id))
    }

    /// Whether a pane's fade is on its way. Render asks for frames while this holds, and the frame
    /// that draws a fade's end turns it false, so an idle app draws nothing.
    pub fn fading(&self) -> bool {
        self.fade.fades.moving()
    }

    /// Head each pane in `shown` for its fade: the setting's strength for all but the focused one
    /// and those that need the user (blocked, an error, a finish not seen yet), whose whole pane
    /// stays in view (docs/architecture/attention.md). A done or error the user saw while the pane
    /// had the focus doesn't hold it bright after they leave; any change while they are away does,
    /// and blocked always does. A pane
    /// whose fade changed gets there over a drawer's slide time, or at once with Reduce Motion.
    pub(super) fn note_fades(&mut self, shown: &[String]) {
        let focused = self.target().map(|(_, id, _)| id);
        let states: Vec<Option<Indicator>> = {
            let m = self.app.model();
            let state = |id: &str| m.block(id).and_then(|(_, b)| status::indicator_of(&b.newest_first()));
            shown.iter().map(|id| state(id)).collect()
        };
        // shortcut: a state that comes and goes between two frames isn't seen as a change, so a
        // second identical finish within ~8 ms of a change counts as seen; a per-record sequence
        // number from the server would close it, if that ever matters
        let seen = &mut self.fade.seen;
        seen.retain(|id, _| shown.contains(id));
        for (id, state) in shown.iter().zip(&states) {
            if self.window_active && focused.as_ref() == Some(id) {
                seen.insert(id.clone(), *state);
            } else if seen.get(id).is_some_and(|s| s != state) {
                seen.remove(id);
            }
        }
        // blocked waits for the user however long they looked; done and error are news, which
        // seeing is enough for
        let needs_user = |id: &String, state: &Option<Indicator>| match state {
            Some(Indicator::Blocked(_)) => true,
            Some(i) => i.needs_user() && self.fade.seen.get(id) != Some(state),
            None => false,
        };
        let panes: Vec<(&str, bool)> =
            shown.iter().zip(&states).map(|(id, state)| (id.as_str(), needs_user(id, state))).collect();
        let targets = fade::targets(&panes, focused.as_deref(), self.look.fade);
        let now = self.motion_now();
        let mut fades = std::mem::take(&mut self.fade.fades);
        fades.aim(targets, now, &|| self.reduce_motion());
        self.fade.fades = fades;
    }
}
