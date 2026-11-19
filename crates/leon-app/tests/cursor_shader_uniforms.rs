//! The uniforms leon gives a cursor shader (docs/cursor-shaders.md): sizes and the cursor in
//! device pixels, the cursor as left/bottom edge with y down, styles as `CURSORSTYLE_*`, colours
//! 0..1, and `iTimeCursorChange` when the cursor last changed.

// macOS only: cursor shaders are built on macOS only for now.
#![cfg(target_os = "macos")]

use std::time::{Duration, Instant};

use leon_app::cursor_shader::{color, Cursor, Drawn, Style, Uniforms};

fn drawn(rect: [f32; 4], cell: (usize, usize), style: Style) -> Drawn {
    Drawn { block: "b1".into(), rect, scale: 2.0, style, cell, rgb: 0xffbf33 }
}

#[test]
fn sizes_and_the_cursor_are_device_pixels_with_y_down() {
    let mut u = Uniforms::default();
    u.set_size(2880., 1800.);
    assert_eq!(u.resolution, [2880., 1800., 1.]);
    assert_eq!(u.channel_resolution[0], [2880., 1800., 1., 0.]);
    let t0 = Instant::now();
    let mut c = Cursor::new(t0);
    c.enable(true, t0);
    c.feed(Some(&drawn([10., 20., 8., 16.], (1, 1), Style::Block)), t0);
    // left 10 pt = 20 px; bottom edge 20 + 16 = 36 pt = 72 px; 8 x 16 pt = 16 x 32 px
    assert_eq!(c.u.current_cursor, [20., 72., 16., 32.]);
    assert_eq!(c.u.cursor_visible, 1);
}

#[test]
fn styles_colours_and_the_change_time() {
    assert_eq!([Style::Block, Style::Hollow, Style::Bar, Style::Underline].map(|s| s as i32), [0, 1, 2, 3]);
    assert_eq!(color(0xff8000), [1.0, 128. / 255., 0.0, 1.0]);
    let t0 = Instant::now();
    let mut c = Cursor::new(t0);
    c.enable(true, t0);
    c.feed(Some(&drawn([10., 20., 8., 16.], (1, 1), Style::Block)), t0);
    let later = t0 + Duration::from_millis(1500);
    assert!(c.feed(Some(&drawn([18., 20., 2., 16.], (1, 2), Style::Bar)), later));
    assert_eq!(c.u.current_cursor_style, 2);
    assert_eq!(c.u.previous_cursor_style, 0);
    assert_eq!(c.u.current_cursor_color, [1.0, 191. / 255., 51. / 255., 1.0]);
    assert!((c.u.cursor_change_time - 1.5).abs() < 1e-3, "{}", c.u.cursor_change_time);
    // the clock the shader sees moves on each frame drawn
    let frame = later + Duration::from_millis(16);
    assert_eq!(c.next(frame), leon_app::cursor_shader::Next::Draw);
    assert!((c.u.time - 1.516).abs() < 1e-3);
    assert!((c.u.time_delta).abs() < 1e-6, "the first frame of a run has no delta");
    assert_eq!(c.u.frame, 1);
}

#[test]
fn the_theme_fills_the_colours_and_the_palette() {
    let t = leon_app::theme::DARK;
    let mut c = Cursor::new(Instant::now());
    c.set_theme(&t);
    assert_eq!(c.u.background_color, color(t.bg));
    assert_eq!(c.u.foreground_color, color(t.fg));
    assert_eq!(c.u.cursor_color, color(t.cursor));
    assert_eq!(c.u.palette[1], color(t.ansi[1]));
    assert_eq!(c.u.palette[16], [0.; 4], "past the 16 ANSI colours the palette is zero");
}
