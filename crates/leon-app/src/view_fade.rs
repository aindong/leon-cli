//! The fade of the panes without the focus (`crate::fade`) in the workspace: each frame heads every
//! shown pane for its fade, and a pane is drawn with the window's theme faded that far.

use super::*;
use crate::fade::{self, Fades};

#[derive(Default)]
pub(super) struct FadeUi {
    fades: Fades,
    /// The time as a test set it; None reads the clock.
    clock: Option<Instant>,
}

impl Workspace {
    /// Fade as if it were `at` (tests).
    pub fn set_fade_clock(&mut self, at: Instant) {
        self.fade.clock = Some(at);
    }

    /// How far pane `id` is faded in the frame last drawn: 0 at full strength, at most the
    /// `fade_inactive_panes` setting.
    pub fn pane_fade(&self, id: &str) -> f32 {
        self.fade.fades.amount(id)
    }

    /// The theme pane `id` is drawn with: the window's, faded as far as the pane is.
    pub fn pane_theme(&self, id: &str) -> Theme {
        fade::theme(&self.theme, self.pane_fade(id))
    }

    /// Whether a pane's fade is on its way. The poll redraws while this holds, and the frame that
    /// draws a fade's end turns it false, so an idle app draws nothing.
    pub fn fading(&self) -> bool {
        self.fade.fades.moving()
    }

    /// Head each pane in `shown` for its fade: the setting's strength for all but the focused one
    /// and those that need the user (blocked, an error, a finish not seen yet), whose whole pane
    /// stays in view (docs/architecture/attention.md). A pane whose fade changed gets there over
    /// a drawer's slide time, or at once with Reduce Motion.
    pub(super) fn note_fades(&mut self, shown: &Panes) {
        let focused = self.target().map(|(_, id, _)| id);
        let targets = {
            let m = self.app.model();
            let needs_user = |id: &str| {
                let state = m.block(id).and_then(|(_, b)| status::indicator_of(&b.newest_first()));
                state.is_some_and(Indicator::needs_user)
            };
            let panes: Vec<(&str, bool)> = shown.iter().map(|id| (id.as_str(), needs_user(id))).collect();
            fade::targets(&panes, focused.as_deref(), self.look.fade)
        };
        let now = self.fade.clock.unwrap_or_else(Instant::now);
        let mut fades = std::mem::take(&mut self.fade.fades);
        fades.aim(targets, now, &|| self.reduce_motion());
        self.fade.fades = fades;
    }
}
