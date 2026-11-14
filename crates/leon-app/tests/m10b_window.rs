//! M10b criterion 13: the first window is generous and centred; the last size and position come
//! back only when they still fit a screen.

use leon_app::chrome::{window_frame, Frame, Settings, MIN_WINDOW};

fn f(x: f32, y: f32, w: f32, h: f32) -> Frame {
    Frame { x, y, w, h }
}

const LAPTOP: Frame = Frame { x: 0., y: 0., w: 1512., h: 982. };

#[test]
fn the_first_window_is_three_quarters_of_the_screen_and_centred() {
    let w = window_frame(None, &[LAPTOP]);
    assert!((w.w - 1134.).abs() < 1. && (w.h - 736.5).abs() < 1., "{w:?}");
    assert!((w.x + w.w / 2. - 756.).abs() < 1. && (w.y + w.h / 2. - 491.).abs() < 1., "centred: {w:?}");
    let big = window_frame(None, &[f(0., 0., 1200., 800.)]);
    assert!(big.w >= 1000. && big.h >= 680., "never under 1000x680 on a screen that fits it: {big:?}");
    let tiny = window_frame(None, &[f(0., 0., 900., 600.)]);
    assert!(tiny.w <= 900. && tiny.h <= 600., "nor bigger than the screen: {tiny:?}");
    assert_eq!(MIN_WINDOW, (720., 440.));
}

#[test]
fn a_saved_frame_comes_back_only_on_a_screen() {
    let saved = f(100., 80., 1300., 850.);
    assert_eq!(window_frame(Some(saved), &[LAPTOP]), saved, "a frame on the screen is restored");
    let second = f(1512., 0., 2560., 1440.);
    let there = f(1700., 100., 1400., 900.);
    assert_eq!(window_frame(Some(there), &[LAPTOP, second]), there, "on a second screen too");
    let gone = window_frame(Some(there), &[LAPTOP]);
    assert_eq!(gone, window_frame(None, &[LAPTOP]), "a screen that is gone: the default instead");
    let mostly_off = f(1300., 700., 1000., 700.);
    assert_eq!(window_frame(Some(mostly_off), &[LAPTOP]), window_frame(None, &[LAPTOP]), "mostly off-screen");
    let small = f(10., 10., 200., 100.);
    let fixed = window_frame(Some(small), &[LAPTOP]);
    assert!(fixed.w >= MIN_WINDOW.0 && fixed.h >= MIN_WINDOW.1, "never under the minimum: {fixed:?}");
    let nan = f(f32::NAN, 0., 1000., 700.);
    assert_eq!(window_frame(Some(nan), &[LAPTOP]), window_frame(None, &[LAPTOP]), "a bad number: the default");
}

#[test]
fn the_window_frame_is_saved_with_the_settings() {
    let dir = tempfile::Builder::new()
        .permissions(std::os::unix::fs::PermissionsExt::from_mode(0o700))
        .tempdir()
        .unwrap();
    let path = dir.path().join("app.json");
    let s = Settings { window: Some(f(40., 50., 1280., 820.)), ..Settings::default() };
    s.save(&path).unwrap();
    assert_eq!(Settings::load(&path).window, Some(f(40., 50., 1280., 820.)));
    assert_eq!(Settings::default().window, None);
}
