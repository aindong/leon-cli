//! Overlay geometry and painting, from the shared client's copy of each window's floats and
//! drawers (`remote::Session::overlays`).
use super::draw::{self, Colors};
use leon_core::overlays::{DockSide, Overlays};
use ratatui::widgets::{Block, Borders, Clear, Widget};
use ratatui::{
    buffer::Buffer,
    layout::{Margin, Rect},
    style::Style,
};

pub struct Pane {
    pub id: String,
    pub outer: Rect,
    pub inner: Rect,
}

pub fn layout(o: &Overlays, area: Rect) -> (Rect, Vec<Pane>) {
    let mut tiles = area;
    let mut out = Vec::new();
    for d in o.docked.iter().filter(|d| !d.hidden) {
        let r = match d.side {
            DockSide::Left | DockSide::Right => {
                let width = ((area.width as f64 * d.size).round() as u16).min(tiles.width);
                let x = if d.side == DockSide::Left { tiles.x } else { tiles.right() - width };
                let r = Rect::new(x, tiles.y, width, tiles.height);
                if d.side == DockSide::Left {
                    tiles.x += width;
                }
                tiles.width -= width;
                r
            }
            DockSide::Bottom => {
                let height = ((area.height as f64 * d.size).round() as u16).min(tiles.height);
                let r = Rect::new(tiles.x, tiles.bottom() - height, tiles.width, height);
                tiles.height -= height;
                r
            }
        };
        out.push(pane(&d.block_id, r));
    }
    for f in o.floating.iter().filter(|f| !f.hidden) {
        let b = f.bounds;
        let x = (b.x * area.width as f64).round() as u16;
        let y = (b.y * area.height as f64).round() as u16;
        let width = ((b.width * area.width as f64).round() as u16).min(area.width.saturating_sub(x));
        let height = ((b.height * area.height as f64).round() as u16).min(area.height.saturating_sub(y));
        out.push(pane(&f.block_id, Rect::new(area.x + x, area.y + y, width, height)));
    }
    (tiles, out)
}
fn pane(id: &str, outer: Rect) -> Pane {
    Pane { id: id.into(), outer, inner: outer.inner(Margin::new(1, 1)) }
}
pub fn paint(buf: &mut Buffer, p: &Pane, focused: bool, colors: Colors) {
    Clear.render(p.outer, buf);
    let color = if focused { draw::brand::AMBER } else { draw::brand::UMBER };
    Block::default().borders(Borders::ALL).border_style(Style::default().fg(colors.rgb(color))).render(p.outer, buf);
}

/// A focused overlay remains above a zoomed tile; otherwise preserve tile zoom's input policy.
pub use leon_app::floats::target as focused;
