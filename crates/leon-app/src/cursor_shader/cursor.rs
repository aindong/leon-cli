//! The cursor a shader sees: what the focused pane's grid painted, turned into the contract's
//! uniforms, and when the overlay draws. No GPU and no GPUI here.

use std::time::{Duration, Instant};

use super::{color, Uniforms};
use crate::theme::Theme;

/// A cursor's style as the contract numbers it (`CURSORSTYLE_*`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Style {
    Block = 0,
    Hollow = 1,
    Bar = 2,
    Underline = 3,
}

/// A cursor a pane's grid painted.
#[derive(Clone, PartialEq, Debug)]
pub struct Drawn {
    /// The pane's block: a cursor in another one is a jump, not a move.
    pub block: String,
    /// Left, top, width, height in window points.
    pub rect: [f32; 4],
    /// Device pixels per point.
    pub scale: f32,
    pub style: Style,
    /// Its cell (line, column): a new rect in the same cell is the pane moving, not the cursor.
    pub cell: (usize, usize),
    /// 0xRRGGBB.
    pub rgb: u32,
}

/// What the overlay does on a display-link tick.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Next {
    /// Draw the shader with these uniforms.
    Draw,
    /// The animation is over (or the overlay was turned off): draw one clear frame.
    Clear,
    /// Nothing to draw: stop the link.
    Stop,
}

/// The uniforms and the animation's end.
#[derive(Clone, Debug)]
pub struct Cursor {
    pub u: Uniforms,
    start: Instant,
    block: Option<String>,
    cell: (usize, usize),
    last_frame: Option<Instant>,
    /// Draw until then; None when idle (the last frame drawn was a clear one).
    pub until: Option<Instant>,
    /// Drawing is wanted: the window is active, Reduce Motion is off, no overview or palette.
    pub enabled: bool,
    /// How long a change animates.
    pub duration: Duration,
}

impl Cursor {
    /// The block whose cursor was fed last.
    pub fn block(&self) -> Option<&str> {
        self.block.as_deref()
    }

    /// `iTime` counts from `start`.
    pub fn new(start: Instant) -> Self {
        Cursor {
            u: Uniforms::default(),
            start,
            block: None,
            cell: (0, 0),
            last_frame: None,
            until: None,
            enabled: false,
            duration: Duration::from_secs(1),
        }
    }

    fn secs(&self, t: Instant) -> f32 {
        t.saturating_duration_since(self.start).as_secs_f32()
    }

    /// The theme's colours: background, foreground, cursor, and the 16 ANSI colours as the
    /// palette's first entries.
    pub fn set_theme(&mut self, t: &Theme) {
        let u = &mut self.u;
        (u.background_color, u.foreground_color, u.cursor_color) = (color(t.bg), color(t.fg), color(t.cursor));
        u.cursor_text = color(t.bg);
        for (p, c) in u.palette.iter_mut().zip(t.ansi) {
            *p = color(c);
        }
    }

    /// Turn drawing on or off at `now` (`iFocus` follows). Off ends a running animation with a
    /// clear frame on the next tick.
    pub fn enable(&mut self, on: bool, now: Instant) {
        if on && !self.enabled {
            self.u.time_focus = self.secs(now);
        }
        self.enabled = on;
        self.u.focus = on as i32;
    }

    /// The cursor the focused pane just painted (None: it painted none, the cursor is hidden).
    /// Returns true when an animation starts and the display link should run.
    pub fn feed(&mut self, drawn: Option<&Drawn>, now: Instant) -> bool {
        let Some(d) = drawn else {
            self.u.cursor_visible = 0;
            return false;
        };
        let s = d.scale;
        // the contract on Metal: y grows down, xy is the cursor's left and bottom edges, in pixels
        let [x, y, w, h] = d.rect;
        let rect = [x * s, (y + h) * s, w * s, h * s];
        let (c, style) = (color(d.rgb), d.style as i32);
        let u = &self.u;
        let shown = u.cursor_visible == 1;
        if shown && rect == u.current_cursor && c == u.current_cursor_color && style == u.current_cursor_style {
            return false;
        }
        let first = self.block.as_deref() != Some(d.block.as_str());
        // the same cell, style and colour drawn somewhere else: the pane moved (a split or float
        // sliding), not the cursor
        let slid = d.cell == self.cell && style == u.current_cursor_style && c == u.current_cursor_color;
        let jump = first || slid;
        let u = &mut self.u;
        u.cursor_visible = 1;
        u.previous_cursor = if jump { rect } else { u.current_cursor };
        u.previous_cursor_color = if jump { c } else { u.current_cursor_color };
        u.previous_cursor_style = if jump { style } else { u.current_cursor_style };
        (u.current_cursor, u.current_cursor_color, u.current_cursor_style) = (rect, c, style);
        self.block = Some(d.block.clone());
        self.cell = d.cell;
        if jump {
            return false;
        }
        self.u.cursor_change_time = self.secs(now);
        if !self.enabled {
            return false;
        }
        self.until = Some(now + self.duration);
        true
    }

    /// What to do on a tick at `now`; for `Draw`, the uniforms' clock moves to `now`.
    pub fn next(&mut self, now: Instant) -> Next {
        match self.until {
            None => Next::Stop,
            Some(t) if now < t && self.enabled => {
                self.u.time_delta = self.last_frame.map_or(0., |l| now.saturating_duration_since(l).as_secs_f32());
                self.u.time = self.secs(now);
                self.u.frame += 1;
                self.last_frame = Some(now);
                Next::Draw
            }
            Some(_) => {
                self.until = None;
                self.last_frame = None;
                Next::Clear
            }
        }
    }
}
