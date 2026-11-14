//! A window's layout: a binary tree of splits with blocks at the leaves (docs/spec.md §1).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::new_id;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    /// Side by side; `before` is on the left.
    Horizontal,
    /// Stacked; `before` is on top.
    Vertical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Side {
    Before,
    After,
}

/// Where focus moves to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Toward {
    Left,
    Right,
    Up,
    Down,
}

#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Node {
    Block {
        block_id: String,
    },
    Split {
        split_id: String,
        direction: Direction,
        /// The `before` side's share, between 0 and 1.
        ratio: f64,
        before: Box<Node>,
        after: Box<Node>,
    },
}

/// Fractions of the window.
#[derive(Debug, Clone, Copy)]
struct Rect {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

const EPS: f64 = 1e-9;

impl Node {
    pub fn block(id: &str) -> Node {
        Node::Block { block_id: id.into() }
    }

    /// The block or split id.
    pub fn id(&self) -> &str {
        match self {
            Node::Block { block_id } => block_id,
            Node::Split { split_id, .. } => split_id,
        }
    }

    /// Block ids in order, before sides first.
    pub fn leaves(&self) -> Vec<&str> {
        match self {
            Node::Block { block_id } => vec![block_id],
            Node::Split { before, after, .. } => [before.leaves(), after.leaves()].concat(),
        }
    }

    /// True if a block or split with this id is in the tree.
    pub fn contains(&self, id: &str) -> bool {
        self.id() == id
            || matches!(self, Node::Split { before, after, .. } if before.contains(id) || after.contains(id))
    }

    /// Put block `new` beside the node `anchor` (a block or a split). False if there is no such node.
    pub fn insert(&mut self, anchor: &str, new: &str, direction: Direction, side: Side, ratio: f64) -> bool {
        if self.id() == anchor {
            let old = std::mem::replace(self, Node::block(""));
            let (before, after) = match side {
                Side::Before => (Node::block(new), old),
                Side::After => (old, Node::block(new)),
            };
            *self = Node::Split {
                split_id: new_id("split"),
                direction,
                ratio,
                before: Box::new(before),
                after: Box::new(after),
            };
            return true;
        }
        match self {
            Node::Block { .. } => false,
            Node::Split { before, after, .. } => {
                before.insert(anchor, new, direction, side, ratio) || after.insert(anchor, new, direction, side, ratio)
            }
        }
    }

    /// The tree without block `id`: a split left with one child becomes that child. None if nothing is left.
    pub fn without(self, id: &str) -> Option<Node> {
        match self {
            Node::Block { ref block_id } if block_id == id => None,
            Node::Block { .. } => Some(self),
            Node::Split { split_id, direction, ratio, before, after } => {
                match (before.without(id), after.without(id)) {
                    (Some(b), Some(a)) => {
                        Some(Node::Split { split_id, direction, ratio, before: Box::new(b), after: Box::new(a) })
                    }
                    (one, None) | (None, one) => one,
                }
            }
        }
    }

    /// Where focus goes when block `id` leaves: the nearest block of its sibling subtree.
    pub fn sibling_leaf(&self, id: &str) -> Option<&str> {
        let Node::Split { before, after, .. } = self else { return None };
        if before.id() == id && matches!(**before, Node::Block { .. }) {
            return Some(after.leaves()[0]);
        }
        if after.id() == id && matches!(**after, Node::Block { .. }) {
            return before.leaves().last().copied();
        }
        before.sibling_leaf(id).or_else(|| after.sibling_leaf(id))
    }

    /// Blocks `a` and `b` trade places (either may be absent, for a swap across windows).
    pub fn swap(&mut self, a: &str, b: &str) {
        match self {
            Node::Block { block_id } => *block_id = swapped(block_id, a, b).to_string(),
            Node::Split { before, after, .. } => {
                before.swap(a, b);
                after.swap(a, b);
            }
        }
    }

    /// Set the ratio of the split that has `before_id` on its before side and `after_id` on its after side.
    pub fn set_ratio(
        &mut self,
        direction: Direction,
        before_id: &str,
        after_id: &str,
        ratio: f64,
    ) -> Result<(), String> {
        let Node::Split { direction: d, ratio: r, before, after, .. } = self else {
            return Err(format!("no split has {before_id} before {after_id}"));
        };
        if before.contains(before_id) && after.contains(after_id) {
            if *d != direction {
                return Err(format!("{before_id} and {after_id} are split {d:?}, not {direction:?}").to_lowercase());
            }
            *r = ratio;
            return Ok(());
        }
        let both = |n: &Node| n.contains(before_id) && n.contains(after_id);
        match (both(before), both(after)) {
            (true, _) => before.set_ratio(direction, before_id, after_id, ratio),
            (_, true) => after.set_ratio(direction, before_id, after_id, ratio),
            _ => Err(format!("no split has {before_id} before {after_id}")),
        }
    }

    /// The block after (or before) `from` in leaf order; `wrap` goes round the ends.
    pub fn step(&self, from: &str, forward: bool, wrap: bool) -> Option<&str> {
        let leaves = self.leaves();
        let i = leaves.iter().position(|l| *l == from)?;
        let n = leaves.len();
        let j = match (forward, wrap) {
            (true, _) if i + 1 < n => i + 1,
            (false, _) if i > 0 => i - 1,
            (_, false) => return None,
            (true, true) => 0,
            (false, true) => n - 1,
        };
        Some(leaves[j])
    }

    /// The block beside `from` in direction `toward`: the one sharing the most edge, the first in
    /// leaf order on a tie. `wrap` looks at the far side of the window when nothing is there.
    pub fn neighbor(&self, from: &str, toward: Toward, wrap: bool) -> Option<&str> {
        let mut all = Vec::new();
        self.rects(Rect { x: 0.0, y: 0.0, w: 1.0, h: 1.0 }, &mut all);
        let f = all.iter().find(|(id, _)| *id == from)?.1;
        // The edge a neighbour's facing side sits on, the window edge to wrap to, and that side.
        let (edge, far, facing): (f64, f64, fn(&Rect) -> f64) = match toward {
            Toward::Right => (f.x + f.w, 0.0, |r| r.x),
            Toward::Left => (f.x, 1.0, |r| r.x + r.w),
            Toward::Down => (f.y + f.h, 0.0, |r| r.y),
            Toward::Up => (f.y, 1.0, |r| r.y + r.h),
        };
        let across = matches!(toward, Toward::Left | Toward::Right);
        let overlap = |r: &Rect| {
            let (a0, a1, b0, b1) =
                if across { (f.y, f.y + f.h, r.y, r.y + r.h) } else { (f.x, f.x + f.w, r.x, r.x + r.w) };
            a1.min(b1) - a0.max(b0)
        };
        let best = |at: f64| {
            let mut best: Option<(&str, f64)> = None;
            for (id, r) in &all {
                let o = overlap(r);
                if *id != from && (facing(r) - at).abs() < EPS && o > EPS && best.is_none_or(|(_, b)| o > b + EPS) {
                    best = Some((id, o));
                }
            }
            best.map(|(id, _)| id)
        };
        best(edge).or_else(|| if wrap { best(far) } else { None })
    }

    fn rects<'a>(&'a self, r: Rect, out: &mut Vec<(&'a str, Rect)>) {
        match self {
            Node::Block { block_id } => out.push((block_id, r)),
            Node::Split { direction, ratio, before, after, .. } => {
                let (b, a) = match direction {
                    Direction::Horizontal => {
                        let w = r.w * ratio;
                        (Rect { w, ..r }, Rect { x: r.x + w, w: r.w - w, ..r })
                    }
                    Direction::Vertical => {
                        let h = r.h * ratio;
                        (Rect { h, ..r }, Rect { y: r.y + h, h: r.h - h, ..r })
                    }
                };
                before.rects(b, out);
                after.rects(a, out);
            }
        }
    }
}

/// `id` with `a` and `b` exchanged.
pub fn swapped<'a>(id: &'a str, a: &'a str, b: &'a str) -> &'a str {
    if id == a {
        b
    } else if id == b {
        a
    } else {
        id
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use Direction::*;
    use Side::*;

    /// A | (B / C) with the B/C split at `bc`.
    fn three(bc: f64) -> Node {
        let mut t = Node::block("A");
        assert!(t.insert("A", "B", Horizontal, After, 0.5));
        assert!(t.insert("B", "C", Vertical, After, bc));
        t
    }

    fn shape(n: &Node) -> String {
        match n {
            Node::Block { block_id } => block_id.clone(),
            Node::Split { direction, before, after, .. } => {
                format!("{}({},{})", if *direction == Horizontal { "h" } else { "v" }, shape(before), shape(after))
            }
        }
    }

    #[test]
    fn insert_beside_blocks_and_splits() {
        let mut t = three(0.5);
        assert_eq!(shape(&t), "h(A,v(B,C))");
        assert!(t.insert("A", "D", Horizontal, Before, 0.3));
        assert_eq!(shape(&t), "h(h(D,A),v(B,C))");
        let Node::Split { split_id, .. } = &t else { panic!() };
        let root = split_id.clone();
        assert!(t.insert(&root, "E", Vertical, After, 0.5));
        assert_eq!(shape(&t), "v(h(h(D,A),v(B,C)),E)");
        assert!(!t.insert("nope", "F", Vertical, After, 0.5));
        assert!(t.contains(&root) && t.contains("E") && !t.contains("F"));
    }

    #[test]
    fn removing_collapses_and_picks_the_sibling() {
        let t = three(0.5);
        assert_eq!(t.sibling_leaf("C"), Some("B"));
        assert_eq!(t.sibling_leaf("B"), Some("C"));
        assert_eq!(t.sibling_leaf("A"), Some("B"), "the first block of the sibling subtree");
        let t = t.without("C").unwrap();
        assert_eq!(shape(&t), "h(A,B)");
        assert_eq!(t.sibling_leaf("B"), Some("A"));
        let t = t.without("A").unwrap();
        assert_eq!(shape(&t), "B");
        assert_eq!(t.without("B"), None);
    }

    #[test]
    fn swap_and_ratios() {
        let mut t = three(0.5);
        t.swap("A", "C");
        assert_eq!(shape(&t), "h(C,v(B,A))");
        t.swap("C", "X"); // X is in another window
        assert_eq!(shape(&t), "h(X,v(B,A))");
        t.set_ratio(Vertical, "B", "A", 0.25).unwrap();
        t.set_ratio(Horizontal, "X", "A", 0.6).unwrap();
        let Node::Split { ratio, after, .. } = &t else { panic!() };
        let Node::Split { ratio: inner, .. } = &**after else { panic!() };
        assert_eq!((*ratio, *inner), (0.6, 0.25));
        assert!(t.set_ratio(Horizontal, "B", "A", 0.5).is_err(), "wrong direction");
        assert!(t.set_ratio(Vertical, "A", "B", 0.5).is_err(), "wrong order");
    }

    #[test]
    fn focus_moves_by_geometry_and_order() {
        let t = three(0.7); // B is taller than C
        assert_eq!(t.neighbor("C", Toward::Up, false), Some("B"));
        assert_eq!(t.neighbor("B", Toward::Left, false), Some("A"));
        assert_eq!(t.neighbor("A", Toward::Right, false), Some("B"), "most shared edge");
        assert_eq!(t.neighbor("A", Toward::Left, false), None);
        assert_eq!(t.neighbor("A", Toward::Left, true), Some("B"), "wrap to the far right");
        assert_eq!(t.neighbor("C", Toward::Down, true), Some("B"));
        assert_eq!(three(0.5).neighbor("A", Toward::Right, false), Some("B"), "a tie goes to the first");
        assert_eq!(t.step("B", true, false), Some("C"));
        assert_eq!(t.step("C", true, false), None);
        assert_eq!(t.step("C", true, true), Some("A"));
        assert_eq!(t.step("A", false, true), Some("C"));
    }
}
