//! Where a window's panes and dividers go: its layout tree (M3) laid out in pixels. Sizes are
//! whole pixels so dividers stay crisp.

use leon_core::layout::{Direction, Node};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Pane {
    pub block_id: String,
    pub rect: Rect,
}

/// The gap between a split's two sides. Dragging it sets the split's ratio (`session.resize_pane`
/// names the split by a block on each side).
#[derive(Debug, Clone, PartialEq)]
pub struct Divider {
    pub rect: Rect,
    pub direction: Direction,
    pub before: String,
    pub after: String,
    /// The whole split; `ratio` is the before side's share of it less the divider.
    pub split: Rect,
    pub ratio: f64,
}

#[derive(Debug, Default, PartialEq)]
pub struct Layout {
    pub panes: Vec<Pane>,
    pub dividers: Vec<Divider>,
}

/// Lay `tree` out in `size` (w, h) with `gap`-wide dividers; a `zoomed` block fills it alone.
pub fn layout(tree: &Node, zoomed: Option<&str>, size: (f32, f32), gap: f32) -> Layout {
    let all = Rect { x: 0., y: 0., w: size.0, h: size.1 };
    let mut out = Layout::default();
    match zoomed.filter(|z| tree.leaves().contains(z)) {
        Some(z) => out.panes.push(Pane { block_id: z.to_string(), rect: all }),
        None => place(tree, all, gap, &mut out),
    }
    out
}

fn place(node: &Node, r: Rect, gap: f32, out: &mut Layout) {
    let Node::Split { direction, ratio, before, after, .. } = node else {
        out.panes.push(Pane { block_id: node.id().to_string(), rect: r });
        return;
    };
    let (b, d, a) = match direction {
        Direction::Horizontal => {
            let room = (r.w - gap).max(0.);
            let w = (room * *ratio as f32).round();
            let at = r.x + w;
            (Rect { w, ..r }, Rect { x: at, w: gap, ..r }, Rect { x: at + gap, w: room - w, ..r })
        }
        Direction::Vertical => {
            let room = (r.h - gap).max(0.);
            let h = (room * *ratio as f32).round();
            let at = r.y + h;
            (Rect { h, ..r }, Rect { y: at, h: gap, ..r }, Rect { y: at + gap, h: room - h, ..r })
        }
    };
    out.dividers.push(Divider {
        rect: d,
        direction: *direction,
        before: before.leaves()[0].to_string(),
        after: after.leaves()[0].to_string(),
        split: r,
        ratio: *ratio,
    });
    place(before, b, gap, out);
    place(after, a, gap, out);
}

/// The channel between two pane tiles, and between the tiles and the pane area's edge.
pub const GAP: f32 = 6.;
pub const INSET: f32 = 6.;
/// A pane header's height, and the space between a tile's edge (or its header) and its text.
pub const HEADER: f32 = 24.;
pub const PADDING: f32 = 8.;

/// `tree` laid out as tiles in an area of `size`: inset from its edge and `GAP` apart. The gaps
/// are the dividers a drag resizes.
pub fn tiles(tree: &Node, zoomed: Option<&str>, size: (f32, f32)) -> Layout {
    let inner = ((size.0 - 2. * INSET).max(0.), (size.1 - 2. * INSET).max(0.));
    let mut l = layout(tree, zoomed, inner, GAP);
    let shift = |r: &mut Rect| (r.x, r.y) = (r.x + INSET, r.y + INSET);
    l.panes.iter_mut().for_each(|p| shift(&mut p.rect));
    for d in &mut l.dividers {
        shift(&mut d.rect);
        shift(&mut d.split);
    }
    l
}

/// Where a tile's terminal text goes: below its header (when headers show), less the padding.
pub fn text_rect(tile: Rect, header: bool) -> Rect {
    let top = if header { HEADER } else { 0. };
    Rect {
        x: tile.x + PADDING,
        y: tile.y + top + PADDING,
        w: (tile.w - 2. * PADDING).max(0.),
        h: (tile.h - top - 2. * PADDING).max(0.),
    }
}
