//! Drawing into ratatui's buffer: a pane's screen as the server sent it, the borders between
//! panes, and the status bar. Ratatui diffs each frame against the last, so the host only gets the
//! cells that changed.

use leon_app::panes::{Divider, Pane};
use leon_core::layout::Direction;
use leon_core::screen::Frame;
use leon_core::theme::DARK;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

/// leon's brand palette (brand/README.md).
pub mod brand {
    pub const IRON: u32 = 0x292521;
    pub const MAROON: u32 = 0x542d35;
    pub const AMBER: u32 = 0xf3a747;
    pub const PARCHMENT: u32 = 0xf5f0e7;
    pub const CHALK: u32 = 0xd8cec2;
    pub const UMBER: u32 = 0x65594f;
    pub const COPPER: u32 = 0xe78155;
    pub const ROSE: u32 = 0xe28e9b;
}

/// How colours go out to the host: 24-bit where it says it takes them (`COLORTERM`), else the
/// host's own 16 colours for the theme's ANSI ones and the nearest xterm-256 entry for the rest.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Colors {
    pub truecolor: bool,
}

impl Colors {
    pub fn from_env() -> Colors {
        let v = std::env::var("COLORTERM").unwrap_or_default().to_ascii_lowercase();
        Colors { truecolor: v == "truecolor" || v == "24bit" }
    }

    /// A chrome colour.
    pub fn rgb(&self, c: u32) -> Color {
        if self.truecolor {
            return Color::Rgb((c >> 16) as u8, (c >> 8) as u8, c as u8);
        }
        Color::Indexed(nearest_256(c))
    }

    /// A colour from a pane's screen: the theme's default fg and bg are the host's defaults, and
    /// without truecolor the theme's ANSI colours are the host's own.
    pub fn pane(&self, c: u32) -> Color {
        if c == DARK.fg || c == DARK.bg {
            return Color::Reset;
        }
        match DARK.ansi.iter().position(|&a| a == c) {
            Some(i) if !self.truecolor => Color::Indexed(i as u8),
            _ => self.rgb(c),
        }
    }
}

/// The xterm-256 entry nearest `c`: the 6×6×6 cube or the gray ramp, whichever is closer.
pub fn nearest_256(c: u32) -> u8 {
    let ch = [(c >> 16) & 0xff, (c >> 8) & 0xff, c & 0xff].map(|v| v as i32);
    const LEVELS: [i32; 6] = [0, 95, 135, 175, 215, 255];
    let level = |v: i32| (0..6).min_by_key(|&i| (LEVELS[i] - v).abs()).unwrap();
    let cube = ch.map(level);
    let dist = |rgb: [i32; 3]| (0..3).map(|i| (rgb[i] - ch[i]).pow(2)).sum::<i32>();
    let cube_rgb = cube.map(|i| LEVELS[i]);
    let avg = ch.iter().sum::<i32>() / 3;
    let gray = ((avg - 8).max(0) / 10).min(23);
    let gray_v = 8 + 10 * gray;
    if dist([gray_v; 3]) < dist(cube_rgb) {
        232 + gray as u8
    } else {
        16 + (36 * cube[0] + 6 * cube[1] + cube[2]) as u8
    }
}

/// Paint `f` into `area`, clipped (a screen another client sized differently shows its top left).
pub fn screen(buf: &mut Buffer, area: Rect, f: &Frame, colors: Colors) {
    let (right, bottom) = (area.x + area.width, area.y + area.height);
    for b in &f.backgrounds {
        let y = area.y as usize + b.line;
        if y >= bottom as usize {
            continue;
        }
        let from = area.x as usize + b.col;
        for x in from..(from + b.len).min(right as usize) {
            buf[(x as u16, y as u16)].set_bg(colors.pane(b.color));
        }
    }
    for (line, row) in f.rows.iter().enumerate().take(area.height as usize) {
        let y = area.y + line as u16;
        for s in row.iter().filter(|s| s.col < area.width as usize) {
            let mut style = Style::default().fg(colors.pane(s.fg));
            for (on, m) in [
                (s.bold, Modifier::BOLD),
                (s.italic, Modifier::ITALIC),
                (s.underline, Modifier::UNDERLINED),
                (s.strikeout, Modifier::CROSSED_OUT),
            ] {
                if on {
                    style = style.add_modifier(m);
                }
            }
            let x = area.x + s.col as u16;
            let room = (right - x) as usize;
            // set_stringn keeps the cell's background (set above) and patches only the style given
            buf.set_stringn(x, y, &s.text, room.min(s.cells), style);
        }
    }
}

/// A pane's cell rect from the layout's float one (its sizes are whole cells already).
pub fn cells(r: &leon_app::panes::Rect, origin: Rect) -> Rect {
    let clamp = |v: f32, max: u16| (v.max(0.).round() as u16).min(max);
    let (x, y) = (clamp(r.x, origin.width), clamp(r.y, origin.height));
    Rect {
        x: origin.x + x,
        y: origin.y + y,
        width: clamp(r.w, origin.width - x),
        height: clamp(r.h, origin.height - y),
    }
}

/// The lines between panes: `│` between side-by-side panes, `─` between stacked ones, in Amber
/// when they touch the focused pane, else Umber.
pub fn borders(
    buf: &mut Buffer,
    area: Rect,
    dividers: &[Divider],
    panes: &[Pane],
    focus: Option<&str>,
    colors: Colors,
) {
    let focused = panes.iter().find(|p| Some(p.block_id.as_str()) == focus).map(|p| cells(&p.rect, area));
    for d in dividers {
        let r = cells(&d.rect, area);
        let touches = focused.is_some_and(|f| {
            let grown =
                Rect { x: f.x.saturating_sub(1), y: f.y.saturating_sub(1), width: f.width + 2, height: f.height + 2 };
            grown.intersects(r)
        });
        let style = Style::default().fg(colors.rgb(if touches { brand::AMBER } else { brand::UMBER }));
        let glyph = if d.direction == Direction::Horizontal { "│" } else { "─" };
        for y in r.y..r.y + r.height {
            for x in r.x..r.x + r.width {
                // where a horizontal border crosses a vertical one, keep the vertical glyph
                if buf[(x, y)].symbol() == "│" && glyph == "─" {
                    continue;
                }
                buf[(x, y)].set_symbol(glyph).set_style(style);
            }
        }
    }
}

/// What the status bar shows.
pub struct Bar<'a> {
    pub session: &'a str,
    /// (label, active) per tab.
    pub tabs: Vec<(String, bool)>,
    /// The prefix was pressed and the next key is the table's.
    pub pending: bool,
    pub prefix: &'a str,
    /// A message (an action failed) shown instead of the hint.
    pub note: Option<&'a str>,
}

/// The one-line bar: the badge, the session, its tabs, and a hint or note on the right.
pub fn bar(buf: &mut Buffer, row: Rect, b: &Bar, colors: Colors) {
    let base = Style::default().bg(colors.rgb(brand::IRON)).fg(colors.rgb(brand::CHALK));
    buf.set_style(row, base);
    let mut x = row.x;
    let mut put = |text: &str, style: Style, buf: &mut Buffer| {
        if x < row.x + row.width {
            x = buf.set_stringn(x, row.y, text, (row.x + row.width - x) as usize, style).0;
        }
    };
    let badge =
        Style::default().bg(colors.rgb(brand::MAROON)).fg(colors.rgb(brand::AMBER)).add_modifier(Modifier::BOLD);
    put(" leon ", badge, buf);
    put(&format!(" {} ", b.session), base.fg(colors.rgb(brand::PARCHMENT)).add_modifier(Modifier::BOLD), buf);
    for (i, (label, active)) in b.tabs.iter().enumerate() {
        let text = format!(" {}:{label} ", i + 1);
        let style = if *active {
            Style::default().bg(colors.rgb(brand::AMBER)).fg(colors.rgb(brand::IRON)).add_modifier(Modifier::BOLD)
        } else {
            base
        };
        put(&text, style, buf);
    }
    let (right, style) = match (b.note, b.pending) {
        (Some(n), _) => (format!(" {n} "), base.fg(colors.rgb(brand::ROSE))),
        (None, true) => (format!(" {} … ", b.prefix), base.fg(colors.rgb(brand::IRON)).bg(colors.rgb(brand::COPPER))),
        (None, false) => {
            (format!(" {} d detach · {} s sessions ", b.prefix, b.prefix), base.fg(colors.rgb(brand::UMBER)))
        }
    };
    let w = right.chars().count() as u16;
    if x + w <= row.x + row.width {
        buf.set_stringn(row.x + row.width - w, row.y, &right, w as usize, style);
    }
}

#[cfg(test)]
#[path = "draw_tests.rs"]
mod tests;
