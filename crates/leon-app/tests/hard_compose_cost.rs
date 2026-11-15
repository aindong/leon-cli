//! Audit A11: composing a scrolled-back view walks the screen's background rects once, not once
//! per row.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use leon_app::render::{BgRect, Frame};
use leon_app::scrollback;
use leon_core::screen::View;

#[test]
fn compose_with_many_background_rects_is_one_pass() {
    let (lines, cols) = (500, 400);
    let backgrounds: Vec<BgRect> =
        (0..200_000).map(|i| BgRect { line: i % lines, col: (i / lines) % cols, len: 1, color: 7 }).collect();
    let view = View { scrolled: 1000, history: 1000, ..View::default() };
    let live = Frame { lines, cols, rows: vec![Vec::new(); lines], backgrounds, view, ..Frame::default() };
    let t = Instant::now();
    // scrolled back 10 lines: 490 screen rows below 10 history rows
    let c = scrollback::compose(&live, Some(990), &HashMap::new());
    let took = t.elapsed();
    assert_eq!(c.missing.len(), 10);
    assert_eq!(c.frame.backgrounds.len(), 200_000 / lines * (lines - 10), "each shown row keeps its rects");
    assert!(c.frame.backgrounds.iter().all(|b| b.line >= 10), "rects move down with their rows");
    assert!(took < Duration::from_millis(200), "compose took {took:?}");
}

#[test]
fn a_huge_scrolled_count_from_a_host_does_not_overflow() {
    let view = View { scrolled: usize::MAX, history: usize::MAX, ..View::default() };
    let backgrounds = vec![BgRect { line: 1, col: 0, len: 1, color: 7 }];
    let live = Frame { lines: 2, cols: 4, rows: vec![Vec::new(); 2], backgrounds, view, ..Frame::default() };
    let c = scrollback::compose(&live, Some(0), &HashMap::new());
    assert!(c.frame.backgrounds.is_empty(), "a screen rect far below the view isn't drawn");
}
