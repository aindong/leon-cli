//! The terminal methods a client uses beyond the screen (docs/architecture.md, "Scrollback in
//! the app"): history ranges, find, selections, links; and mouse events for `block.event`.

use leon_core::mouse::{self, Action, Button};
use leon_core::screen::Lines;
use leon_core::search::{Kind, Link, Pos, Query, Selected};
use leon_core::theme;

use super::*;

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LinesArgs {
    /// The first line's absolute number (the screen's top row is `scrolled`).
    pub from: usize,
    /// How many lines, at most 1000.
    pub count: usize,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SelectionArgs {
    pub kind: Kind,
    /// Where the selection started and where it reaches now, by absolute line.
    pub anchor: Pos,
    pub head: Pos,
    /// Also return the text it copies.
    #[serde(default)]
    pub text: bool,
}

/// The most wheel notches one event sends.
const MAX_NOTCHES: u32 = 100;

pub(super) fn methods() -> Vec<TermMethod> {
    vec![
        term_method(
            "lines",
            "Rows by absolute line number, from the scrollback and the screen, as `block.watch` sends \
             them; lines no longer kept are left out. With the numbering they were read under.",
            |t, a: LinesArgs| -> Result<Lines, ApiError> { Ok(t.lines(a.from, a.count, &theme::DARK)) },
        ),
        term_method(
            "find",
            "Every match of a query in the scrollback and the screen, oldest first; a match may cross a \
             wrapped line. A bad regex is a 400.",
            |t, q: Query| t.find(&q).map_err(ApiError::bad_request),
        ),
        term_method(
            "selection",
            "A selection's range (words, lines across wraps, a rectangle; half a wide character takes the \
             whole one) and, if asked, the text it copies; null when `anchor` isn't kept.",
            |t, a: SelectionArgs| -> Result<Option<Selected>, ApiError> {
                Ok(t.selection(a.kind, a.anchor, a.head, a.text))
            },
        ),
        term_method(
            "link_at",
            "The link on a cell: an OSC 8 hyperlink, else a URL in its line (across wraps); null if none.",
            |t, at: Pos| -> Result<Option<Link>, ApiError> { Ok(t.link_at(at)) },
        ),
    ]
}

/// A mouse event for the program, encoded by the mouse mode it has now (nothing when it asked for
/// none); the wheel as `lines` notches (up when positive).
pub(super) fn mouse_event(t: &Terminal, ev: InputEvent) -> Result<(), ApiError> {
    let io = |e: std::io::Error| ApiError::new(500, "io", e.to_string());
    match ev {
        InputEvent::Mouse { action, button, line, col, mods } => {
            t.mouse(&mouse::Event { action, button, line, col, mods }).map_err(io)?;
        }
        InputEvent::Wheel { lines, line, col, mods } => {
            let button = if lines > 0 { Button::WheelUp } else { Button::WheelDown };
            let e = mouse::Event { action: Action::Press, button: Some(button), line, col, mods };
            for _ in 0..lines.unsigned_abs().min(MAX_NOTCHES) {
                t.mouse(&e).map_err(io)?;
            }
        }
        _ => {}
    }
    Ok(())
}
