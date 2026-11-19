//! Review findings on pane changes: a cursor in another pane with the same rect is still that
//! pane's (its next move animates), and a jump to another pane ends a running animation.

// macOS only: cursor shaders are built on macOS only for now.
#![cfg(target_os = "macos")]

use std::time::{Duration, Instant};

use leon_app::cursor_shader::{Cursor, Drawn, Next, Style};

fn at(block: &str, x: f32, cell: (usize, usize)) -> Drawn {
    Drawn { block: block.into(), rect: [x, 40., 8., 16.], scale: 1.0, style: Style::Block, cell, rgb: 0xffffff }
}

#[test]
fn another_pane_at_the_same_rect_is_that_panes() {
    let t0 = Instant::now();
    let mut c = Cursor::new(t0);
    c.enable(true, t0);
    c.feed(Some(&at("a", 10., (2, 1))), t0);
    c.feed(Some(&at("b", 10., (2, 1))), t0); // b's cursor where a's was (a zoom, a tab switch)
    assert!(c.feed(Some(&at("b", 18., (2, 2))), t0), "FEED_PANE_BUG: b's first move animates");
}

#[test]
fn a_jump_to_another_pane_ends_a_running_animation() {
    let t0 = Instant::now();
    let mut c = Cursor::new(t0);
    c.enable(true, t0);
    c.feed(Some(&at("a", 10., (2, 1))), t0);
    assert!(c.feed(Some(&at("a", 18., (2, 2))), t0));
    c.feed(Some(&at("b", 300., (0, 0))), t0 + Duration::from_millis(50));
    assert_eq!(c.next(t0 + Duration::from_millis(66)), Next::Clear, "FEED_PANE_BUG: no trail goes on in b");
}
