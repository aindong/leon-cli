//! What the focused pane's cursor does to the overlay: a move in the same pane animates for the
//! set duration; a cursor in another pane, or the same cell drawn elsewhere (the pane sliding), is
//! a jump with no animation; a hidden cursor keeps its place; the same cursor again is nothing;
//! drawing off ends with one clear frame.

// macOS only: cursor shaders are built on macOS only for now.
#![cfg(target_os = "macos")]

use std::time::{Duration, Instant};

use leon_app::cursor_shader::{Cursor, Drawn, Next, Style};

fn at(block: &str, x: f32, cell: (usize, usize)) -> Drawn {
    Drawn { block: block.into(), rect: [x, 40., 8., 16.], scale: 1.0, style: Style::Block, cell, rgb: 0xffffff }
}

fn on(t0: Instant) -> Cursor {
    let mut c = Cursor::new(t0);
    c.enable(true, t0);
    c.duration = Duration::from_millis(500);
    c
}

#[test]
fn a_move_in_the_same_pane_animates_for_the_duration() {
    let t0 = Instant::now();
    let mut c = on(t0);
    assert!(!c.feed(Some(&at("a", 10., (2, 1))), t0), "the first cursor of a pane is a jump");
    assert_eq!(c.next(t0), Next::Stop, "nothing to draw");
    assert!(c.feed(Some(&at("a", 18., (2, 2))), t0), "a move starts the link");
    assert_eq!(c.u.previous_cursor[0], 10.);
    assert_eq!(c.u.current_cursor[0], 18.);
    assert_eq!(c.next(t0 + Duration::from_millis(100)), Next::Draw);
    assert_eq!(c.next(t0 + Duration::from_millis(499)), Next::Draw);
    assert_eq!(c.next(t0 + Duration::from_millis(500)), Next::Clear, "one clear frame at the end");
    assert_eq!(c.next(t0 + Duration::from_millis(516)), Next::Stop, "then the link stops");
}

#[test]
fn another_pane_is_a_jump() {
    let t0 = Instant::now();
    let mut c = on(t0);
    c.feed(Some(&at("a", 10., (2, 1))), t0);
    assert!(!c.feed(Some(&at("b", 300., (0, 0))), t0), "no trail across panes");
    assert_eq!(c.u.previous_cursor, c.u.current_cursor);
    assert_eq!(c.next(t0), Next::Stop);
}

#[test]
fn the_same_cell_drawn_elsewhere_is_the_pane_sliding() {
    let t0 = Instant::now();
    let mut c = on(t0);
    c.feed(Some(&at("a", 10., (2, 1))), t0);
    for (i, x) in [30., 60., 90.].into_iter().enumerate() {
        let t = t0 + Duration::from_millis(16 * i as u64);
        assert!(!c.feed(Some(&at("a", x, (2, 1))), t), "a slide frame at {x} is not a move");
        assert_eq!(c.u.previous_cursor, c.u.current_cursor);
    }
    assert_eq!(c.next(t0 + Duration::from_millis(60)), Next::Stop, "a slide draws nothing");
    assert!(c.feed(Some(&at("a", 98., (2, 2))), t0 + Duration::from_millis(70)), "a real move after it does");
    assert_eq!(c.u.previous_cursor[0], 90.);
}

#[test]
fn the_same_cursor_again_is_nothing() {
    let t0 = Instant::now();
    let mut c = on(t0);
    c.feed(Some(&at("a", 10., (2, 1))), t0);
    c.feed(Some(&at("a", 18., (2, 2))), t0);
    let change = c.u.cursor_change_time;
    assert!(!c.feed(Some(&at("a", 18., (2, 2))), t0 + Duration::from_millis(300)));
    assert_eq!(c.u.cursor_change_time, change, "a repaint is not a change");
}

#[test]
fn a_hidden_cursor_keeps_its_place_and_showing_it_elsewhere_moves() {
    let t0 = Instant::now();
    let mut c = on(t0);
    c.feed(Some(&at("a", 10., (2, 1))), t0);
    assert!(!c.feed(None, t0));
    assert_eq!(c.u.cursor_visible, 0);
    assert_eq!(c.u.current_cursor[0], 10., "its place is kept");
    assert!(c.feed(Some(&at("a", 50., (2, 6))), t0 + Duration::from_millis(50)), "shown somewhere else: a move");
    assert_eq!(c.u.cursor_visible, 1);
}

#[test]
fn off_draws_nothing_and_ends_a_running_animation() {
    let t0 = Instant::now();
    let mut c = on(t0);
    c.feed(Some(&at("a", 10., (2, 1))), t0);
    assert!(c.feed(Some(&at("a", 18., (2, 2))), t0));
    c.enable(false, t0 + Duration::from_millis(50));
    assert_eq!(c.u.focus, 0);
    assert_eq!(c.next(t0 + Duration::from_millis(66)), Next::Clear, "turned off mid-animation: a clear frame");
    assert_eq!(c.next(t0 + Duration::from_millis(82)), Next::Stop);
    assert!(!c.feed(Some(&at("a", 26., (2, 3))), t0 + Duration::from_millis(90)), "off: a move starts nothing");
    c.enable(true, t0 + Duration::from_millis(100));
    assert_eq!(c.u.focus, 1);
    assert!((c.u.time_focus - 0.1).abs() < 1e-3, "iTimeFocus is when it came back");
}
