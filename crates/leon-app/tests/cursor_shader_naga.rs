//! Workaround 1: naga 25's MSL writer gives `float iChannelTime[4]` in a std140 uniform block a
//! stride of 4 bytes where std140 says 16, so every uniform after it is read 48 bytes off and
//! the cursor lands somewhere else. leon's prefix keeps it out of the block (a zero constant, as
//! the contract's host never sets it). The second test shows naga still has the bug: when it
//! fails, naga is fixed and the workaround can go.

// macOS only: the shaders are compiled with Metal at runtime and drawn on the GPU.
#![cfg(target_os = "macos")]

mod cursor_shader_support;

use cursor_shader_support::*;
use leon_app::cursor_shader::{compile, hoist_globals, msl, Uniforms, Wrapped, PREFIX};

/// `float4()` drawn in one pass over the picture with `prefix` before it.
fn draw(prefix: &str, u: &Uniforms) -> Vec<u8> {
    let (body, inits) = hoist_globals(&float4());
    assert!(inits.is_empty());
    let text = format!("{prefix}{body}\nvoid main() {{\n    mainImage(leon_frag_color, gl_FragCoord.xy);\n}}\n");
    let p = compile(&msl(&Wrapped::raw(text)).expect("translates")).expect("compiles");
    p.render((W, H), u, Some(&picture()))
}

/// The cursor's colour where the cursor is, and the picture elsewhere.
fn cursor_drawn(img: &[u8]) -> bool {
    let inside = at(img, 44, 24); // in [40, 48) x [16, 30)
    let outside = at(img, 10, 10);
    let red = [51, 26, 230, 255]; // vec4(0.9, 0.1, 0.2, 1) as BGRA8
    let close = |a: [u8; 4], b: [u8; 4]| a.iter().zip(b).all(|(x, y)| x.abs_diff(y) <= 2);
    close(inside, red) && close(outside, at(&picture(), 10, 10))
}

#[test]
fn leons_prefix_reads_the_cursor_where_it_is() {
    assert!(!PREFIX.split("uniform Globals").nth(1).unwrap().split("};").next().unwrap().contains("iChannelTime"));
    assert!(cursor_drawn(&draw(PREFIX, &moved())), "the cursor rect is drawn at the cursor");
}

#[test]
fn naga_still_misreads_a_float_array_in_the_block() {
    let naive = PREFIX
        .replace("const float iChannelTime[4] = float[4](0.0, 0.0, 0.0, 0.0);\n", "")
        .replace("    int   iFrame;\n", "    int   iFrame;\n    float iChannelTime[4];\n");
    assert_ne!(naive, PREFIX);
    assert!(
        !cursor_drawn(&draw(&naive, &moved())),
        "with iChannelTime in the block the cursor is read wrong; if this fails, naga is fixed and \
         the prefix can keep the contract's block as it is"
    );
}
