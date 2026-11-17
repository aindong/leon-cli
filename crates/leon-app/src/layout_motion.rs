//! Panes moving from one layout to the next (a split, a close, a zoom): where each pane is part
//! way, where a new pane grows from and a closing one shrinks to, and how long it takes. GPUI-free,
//! like `floats`, whose clock it shares (the workspace's `motion_now`).

use std::time::Duration;

use crate::panes::Rect;

/// How long the panes take to reach a new layout.
pub const LAYOUT: Duration = Duration::from_millis(160);

/// How long a tab or session switch's cross-fade takes: short, so ⌘1..9 never feels slow.
pub const SWITCH: Duration = Duration::from_millis(100);

/// The curve a motion follows: fast then settling for what arrives (a split, a zoom), slow then
/// fast for what leaves (a close, an unzoom).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Ease {
    Out,
    In,
}

/// The part of a motion `elapsed` into one of `length` done, eased; None once it is over.
pub fn progress(elapsed: Duration, length: Duration, ease: Ease) -> Option<f32> {
    if elapsed >= length {
        return None;
    }
    let t = elapsed.as_secs_f32() / length.as_secs_f32();
    Some(match ease {
        Ease::Out => 1. - (1. - t).powi(3),
        Ease::In => t.powi(3),
    })
}

/// `a` moved `p` (0–1) of the way to `b`.
pub fn between(a: Rect, b: Rect, p: f32) -> Rect {
    let m = |x: f32, y: f32| x + (y - x) * p;
    Rect { x: m(a.x, b.x), y: m(a.y, b.y), w: m(a.w, b.w), h: m(a.h, b.h) }
}

/// `r` folded to nothing against the side it shares with `within`, on the axis where it is
/// narrower than `within`: where a pane split out of `within` grows from, and where a pane whose
/// space `within` takes over shrinks to. A right half folds to `within`'s right edge, a lower half
/// to its bottom.
pub fn collapse(r: Rect, within: Rect) -> Rect {
    const NEAR: f32 = 0.5;
    if r.w < within.w - NEAR {
        let right = (r.x + r.w - (within.x + within.w)).abs() < NEAR && (r.x - within.x).abs() >= NEAR;
        let x = if right { r.x + r.w } else { r.x };
        Rect { x, w: 0., ..r }
    } else {
        let bottom = (r.y + r.h - (within.y + within.h)).abs() < NEAR && (r.y - within.y).abs() >= NEAR;
        let y = if bottom { r.y + r.h } else { r.y };
        Rect { y, h: 0., ..r }
    }
}

/// Which of `rects` holds `r`'s middle, if any.
pub fn holding<'a>(r: Rect, rects: impl IntoIterator<Item = (&'a str, Rect)>) -> Option<(&'a str, Rect)> {
    let (cx, cy) = (r.x + r.w / 2., r.y + r.h / 2.);
    rects.into_iter().find(|(_, o)| cx >= o.x && cx <= o.x + o.w && cy >= o.y && cy <= o.y + o.h)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect { x, y, w, h }
    }

    #[test]
    fn eased_and_over_at_the_length() {
        let half = progress(LAYOUT / 2, LAYOUT, Ease::Out).unwrap();
        assert!(half > 0.5, "out: past half way at half the time: {half}");
        let half = progress(LAYOUT / 2, LAYOUT, Ease::In).unwrap();
        assert!(half < 0.5, "in: short of half way at half the time: {half}");
        assert_eq!(progress(Duration::ZERO, LAYOUT, Ease::Out), Some(0.));
        assert_eq!(progress(LAYOUT, LAYOUT, Ease::Out), None);
    }

    #[test]
    fn about_120_to_220_ms_eased_out_on_enter_and_in_on_leave() {
        assert!((120..=220).contains(&LAYOUT.as_millis()), "LAYOUT {LAYOUT:?}");
        assert!(SWITCH < Duration::from_millis(120), "SWITCH {SWITCH:?}");
        let quarter = |e| progress(LAYOUT / 4, LAYOUT, e).unwrap();
        assert!(quarter(Ease::Out) > 0.25, "arriving: past a quarter at a quarter of the time");
        assert!(quarter(Ease::In) < 0.25, "leaving: short of a quarter at a quarter of the time");
    }

    #[test]
    fn between_moves_every_side() {
        assert_eq!(between(r(0., 0., 100., 50.), r(100., 50., 0., 10.), 0.5), r(50., 25., 50., 30.));
    }

    #[test]
    fn a_split_half_folds_to_its_outer_edge() {
        let whole = r(0., 0., 200., 100.);
        assert_eq!(collapse(r(100., 0., 100., 100.), whole), r(200., 0., 0., 100.), "a right half: its right edge");
        assert_eq!(collapse(r(0., 0., 100., 100.), whole), r(0., 0., 0., 100.), "a left half: its left edge");
        assert_eq!(collapse(r(0., 50., 200., 50.), whole), r(0., 100., 200., 0.), "a lower half: the bottom");
        assert_eq!(collapse(r(0., 0., 200., 50.), whole), r(0., 0., 200., 0.), "an upper half: the top");
    }

    #[test]
    fn the_rect_holding_a_middle() {
        let rects = [("a", r(0., 0., 100., 100.)), ("b", r(100., 0., 100., 100.))];
        assert_eq!(holding(r(120., 10., 60., 60.), rects).map(|(id, _)| id), Some("b"));
        assert_eq!(holding(r(300., 10., 10., 10.), rects), None);
    }
}
