//! Each fixture cursor shader drawn by the overlay's path (the matte: two passes over flat black
//! and white, no picture) and composited premultiplied over a picture, as Core Animation does,
//! matches the contract's one-pass draw over that picture.

// macOS only: the shaders are compiled with Metal at runtime and drawn on the GPU.
#![cfg(target_os = "macos")]

mod cursor_shader_support;

use cursor_shader_support::*;
use leon_app::cursor_shader::{compile, translate, Mode};

/// `src` through the overlay, over the picture, against the contract's draw of it.
fn check(name: &str, src: &str, tolerance: u8) {
    let u = moved();
    let want = contract(src, &u);
    let layer = overlay(src, &u);
    let got = over_picture(&layer);
    let drew = layer.chunks(4).filter(|p| p[3] > 0).count();
    assert!(drew > 0, "{name}: the overlay drew something");
    assert_ne!(want, picture(), "{name}: the contract's draw changes the picture");
    let (max, off) = diff(&got, &want);
    assert!(max <= tolerance, "{name}: the overlay is {max}/255 off the contract ({off} pixels past 3/255)");
}

/// How far a glow past 1.0 may be off (0.4 of a full glow over the picture's colours).
const SATURATED_GLOW_SLACK: u8 = 64;

#[test]
fn a_smear_matches_the_contract() {
    check("smear", &smear(), 3);
}

#[test]
fn a_glow_over_a_saturated_colour_is_close() {
    // mixing toward red 1.4 saturates the black pass first, so the matte over-reads coverage by up
    // to 0.4 of the glow; the same glow within 0..1 is exact
    check("glow", GLOW, SATURATED_GLOW_SLACK);
    check("glow within 1.0", &GLOW.replace("vec3(1.4, 0.6, 0.2)", "vec3(1.0, 0.6, 0.2)"), 3);
}

#[test]
fn a_global_initializer_matches() {
    check("global", &global(), 3);
}

#[test]
fn a_float4_uniform_read_matches() {
    check("float4", &float4(), 3);
}

#[test]
fn reads_of_ichannel0_match() {
    check("channel", &channel(), 3);
}

#[test]
fn the_same_source_compiles_once() {
    let msl = translate(&smear(), Mode::Matte).unwrap();
    let (a, b) = (compile(&msl).unwrap(), compile(&msl).unwrap());
    assert!(std::sync::Arc::ptr_eq(&a, &b), "a pipeline is cached by its source");
    let other = compile(&translate(GLOW, Mode::Matte).unwrap()).unwrap();
    assert!(!std::sync::Arc::ptr_eq(&a, &other), "another source is another pipeline");
}
