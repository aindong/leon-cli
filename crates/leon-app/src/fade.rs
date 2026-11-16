//! Fading the panes that don't have the focus, so the eye rests on the one that does: how far each
//! pane should fade, how it gets there, and the colours a faded pane is drawn with. GPUI-free,
//! like `floats`, whose slide it moves by.

use std::collections::HashMap;
use std::time::Instant;

use crate::floats::slide;
use crate::render::Frame;
use crate::theme::{mix, Theme};

/// How far each shown pane should fade (0 is full strength): `strength` for every pane but the
/// focused one and those that need the user (`shown`'s flag). Nothing fades when a pane is alone,
/// or when the focus isn't on a pane shown, so a window is never faded all over.
pub fn targets(shown: &[(&str, bool)], focused: Option<&str>, strength: f32) -> Vec<(String, f32)> {
    let on = shown.len() > 1 && shown.iter().any(|(id, _)| Some(*id) == focused);
    let of = |id: &str, needs_user: bool| if on && !needs_user && Some(id) != focused { strength } else { 0. };
    shown.iter().map(|(id, needs_user)| (id.to_string(), of(id, *needs_user))).collect()
}

/// `t` as a pane faded by `a` is drawn: every colour moved `a` of the way to the window's own
/// (`chrome`), but the status colours, which a fade never weakens.
pub fn theme(t: &Theme, a: f32) -> Theme {
    let m = |c: u32| mix(c, t.chrome, a);
    Theme {
        fg: m(t.fg),
        bg: m(t.bg),
        cursor: m(t.cursor),
        ansi: t.ansi.map(m),
        chrome: t.chrome,
        hover: m(t.hover),
        border: m(t.border),
        text: m(t.text),
        title: m(t.title),
        muted: m(t.muted),
        accent: m(t.accent),
        selected: m(t.selected),
        brand: m(t.brand),
        working: t.working,
        done: t.done,
        blocked: t.blocked,
        error: t.error,
        selection: m(t.selection),
        found: m(t.found),
        link: m(t.link),
    }
}

/// A screen with its own colours (the server resolved them) moved `a` of the way to `to`.
pub fn frame(mut f: Frame, to: u32, a: f32) -> Frame {
    if a > 0. {
        f.rows.iter_mut().flatten().for_each(|s| s.fg = mix(s.fg, to, a));
        f.backgrounds.iter_mut().for_each(|b| b.color = mix(b.color, to, a));
    }
    f
}

/// One pane's fade on its way from `from` to `to` since `at`, and where the last `aim` found it.
#[derive(Debug, Clone, Copy)]
struct Step {
    from: f32,
    to: f32,
    at: Instant,
    now: f32,
}

/// Each shown pane's fade, which moves to a new target over a slide's time.
#[derive(Debug, Default)]
pub struct Fades {
    panes: HashMap<String, Step>,
}

impl Fades {
    /// Head each pane for its target, as of `now`. A pane seen for the first time is there at
    /// once (opening a tab moves nothing), and so is every pane with Reduce Motion (asked only
    /// when a target changed); a pane whose target changed sets off from where it is, so a fade
    /// turned back never jumps. Panes not in `targets` are forgotten.
    pub fn aim(&mut self, targets: Vec<(String, f32)>, now: Instant, reduce_motion: &dyn Fn() -> bool) {
        let was = std::mem::take(&mut self.panes);
        for (id, to) in targets {
            let rest = Step { from: to, to, at: now, now: to };
            let step = match was.get(&id) {
                Some(s) if s.to != to && !reduce_motion() => Step { from: s.now, to, at: now, now: s.now },
                Some(s) if s.to == to && s.from != to => match slide(now.saturating_duration_since(s.at), false) {
                    Some(done) => Step { now: s.from + (to - s.from) * done, ..*s },
                    None => rest,
                },
                _ => rest,
            };
            self.panes.insert(id, step);
        }
    }

    /// How far pane `id` was faded at the last `aim`; a pane it doesn't know is at full strength.
    pub fn amount(&self, id: &str) -> f32 {
        self.panes.get(id).map_or(0., |s| s.now)
    }

    /// Whether a fade is on its way: false from the `aim` that found the last one at its target.
    pub fn moving(&self) -> bool {
        self.panes.values().any(|s| s.from != s.to)
    }
}
