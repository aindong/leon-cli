//! Mouse reports for programs that ask for them: xterm's modes (X10 `?9`, 1000 click, 1002 drag,
//! 1003 motion) in the SGR (1006), UTF-8 (1005) or legacy byte encodings, and the wheel as arrow
//! keys on the alternate screen (1007). The server encodes by the mode the program has when the
//! event arrives, the way it wraps a paste.

use alacritty_terminal::term::TermMode;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Which mouse events a program asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Report {
    /// `?9`: presses only, no modifiers.
    X10,
    /// `?1000`: presses and releases.
    Click,
    /// `?1002`: those, and moves while a button is down.
    Drag,
    /// `?1003`: every move.
    Motion,
}

/// A program's mouse modes at one moment.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Mode {
    pub report: Option<Report>,
    /// `?1006`.
    pub sgr: bool,
    /// `?1005`.
    pub utf8: bool,
    /// The alternate screen with `?1007` on: with no report mode, the wheel becomes arrow keys.
    pub alt_scroll: bool,
    /// Cursor keys in application mode (DECCKM), for those arrow keys.
    pub app_cursor: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Button {
    Left,
    Middle,
    Right,
    WheelUp,
    WheelDown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Action {
    Press,
    Release,
    Move,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct Mods {
    pub shift: bool,
    pub alt: bool,
    pub ctrl: bool,
}

/// One mouse event on a cell of the screen (0-based; the screen, not the scrollback).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Event {
    pub action: Action,
    /// None: a move with no button down.
    pub button: Option<Button>,
    pub line: usize,
    pub col: usize,
    pub mods: Mods,
}

/// The report mode alacritty's flags name, else X10 when the program set `?9` (`terminal::Escapes`).
pub fn report(mode: TermMode, x10: bool) -> Option<Report> {
    [
        (TermMode::MOUSE_MOTION, Report::Motion),
        (TermMode::MOUSE_DRAG, Report::Drag),
        (TermMode::MOUSE_REPORT_CLICK, Report::Click),
    ]
    .into_iter()
    .find(|(flag, _)| mode.contains(*flag))
    .map(|(_, r)| r)
    .or(x10.then_some(Report::X10))
}

impl Mode {
    pub fn of(mode: TermMode, x10: bool) -> Mode {
        Mode {
            report: report(mode, x10),
            sgr: mode.contains(TermMode::SGR_MOUSE),
            utf8: mode.contains(TermMode::UTF8_MOUSE),
            alt_scroll: mode.contains(TermMode::ALT_SCREEN | TermMode::ALTERNATE_SCROLL),
            app_cursor: mode.contains(TermMode::APP_CURSOR),
        }
    }
}

/// The legacy encoding's coordinates are one byte, 32 + 1 + position.
const LEGACY_MAX: usize = 255 - 33;
/// UTF-8 coordinates go up to what two bytes hold.
const UTF8_MAX: usize = 0x7ff - 33;

/// The bytes the program reads for `e`, or None when its mode doesn't ask for this event.
pub fn encode(e: &Event, m: &Mode) -> Option<Vec<u8>> {
    let wheel = matches!(e.button, Some(Button::WheelUp | Button::WheelDown));
    let Some(report) = m.report else {
        // the wheel on the alternate screen scrolls the program, an arrow key per notch
        return (wheel && m.alt_scroll && e.action == Action::Press).then(|| {
            let way = if e.button == Some(Button::WheelUp) { 'A' } else { 'B' };
            format!("\x1b{}{way}", if m.app_cursor { 'O' } else { '[' }).into_bytes()
        });
    };
    let wanted = match (e.action, report) {
        (Action::Press, _) => true,
        (Action::Release, Report::X10) => false,
        (Action::Release, _) => !wheel,
        (Action::Move, Report::Motion) => true,
        (Action::Move, Report::Drag) => e.button.is_some(),
        (Action::Move, _) => false,
    };
    if !wanted {
        return None;
    }
    let mut code = match e.button {
        Some(Button::Left) => 0,
        Some(Button::Middle) => 1,
        Some(Button::Right) => 2,
        Some(Button::WheelUp) => 64,
        Some(Button::WheelDown) => 65,
        None => 3,
    };
    if e.action == Action::Move {
        code += 32;
    }
    if report != Report::X10 {
        code += 4 * e.mods.shift as u32 + 8 * e.mods.alt as u32 + 16 * e.mods.ctrl as u32;
    }
    if m.sgr {
        let end = if e.action == Action::Release { 'm' } else { 'M' };
        return Some(
            format!("\x1b[<{code};{};{}{end}", e.col.saturating_add(1), e.line.saturating_add(1)).into_bytes(),
        );
    }
    if e.action == Action::Release {
        code = 3 + (code & !3); // the legacy forms can't say which button went up
    }
    let mut out = b"\x1b[M".to_vec();
    out.push(32 + code as u8);
    for pos in [e.col, e.line] {
        if m.utf8 {
            if pos > UTF8_MAX {
                return None;
            }
            let mut buf = [0; 4];
            out.extend(char::from_u32(33 + pos as u32)?.encode_utf8(&mut buf).as_bytes());
        } else {
            if pos > LEGACY_MAX {
                return None;
            }
            out.push(33 + pos as u8);
        }
    }
    Some(out)
}
