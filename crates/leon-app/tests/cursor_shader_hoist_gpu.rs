//! Workaround 2: naga 25 evaluates a global's non-constant initializer (`vec4 C =
//! vec4(iCurrentCursorColor.rgb, 1.0);`) in the entry point but never stores it, so the global
//! stays zero and the shader draws nothing, silently. leon's wrapper declares it bare and assigns
//! it at the start of `main`. The second test shows naga still does it.

// macOS only: the shaders are compiled with Metal at runtime and drawn on the GPU.
#![cfg(target_os = "macos")]

mod cursor_shader_support;

use cursor_shader_support::*;
use leon_app::cursor_shader::{compile, msl, translate, Mode, Wrapped, PREFIX};

fn render(msl_text: &str) -> Vec<u8> {
    compile(msl_text).expect("compiles").render((W, H), &moved(), Some(&picture()))
}

/// The cursor's colour (amber) inside the cursor rect.
fn amber_at_cursor(img: &[u8]) -> bool {
    let [b, g, r, _] = at(img, 44, 24);
    r.abs_diff(255) <= 2 && g.abs_diff(191) <= 2 && b.abs_diff(51) <= 2
}

#[test]
fn a_hoisted_global_draws_its_colour() {
    let img = render(&translate(&global(), Mode::Contract).expect("translates"));
    assert!(amber_at_cursor(&img), "TRAIL_COLOR is the cursor's colour: {:?}", at(&img, 44, 24));
}

#[test]
fn naga_still_drops_a_global_initializer() {
    let text = format!("{PREFIX}{}\nvoid main() {{\n    mainImage(leon_frag_color, gl_FragCoord.xy);\n}}\n", global());
    let img = render(&msl(&Wrapped::raw(text)).expect("translates"));
    assert!(
        !amber_at_cursor(&img),
        "unhoisted, TRAIL_COLOR stays zero; if this fails, naga stores initializers now and the \
         hoist can go"
    );
}
