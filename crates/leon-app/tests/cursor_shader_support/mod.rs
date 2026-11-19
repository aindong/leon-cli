//! Cursor shaders written for leon's tests (none is a third party's), the uniforms of a cursor
//! that just moved, and the render through the overlay's path (the matte, composited
//! premultiplied as Core Animation does) beside the contract's one-pass render.
#![allow(dead_code)]

use leon_app::cursor_shader::{compile, translate, Mode, Uniforms};

pub const W: u64 = 64;
pub const H: u64 = 48;

/// A rect as the contract gives it: (x, y) is the left and bottom edge, y down.
pub const BOX: &str = "
float leon_box(vec2 p, vec4 r) {
    vec2 lo = vec2(r.x, r.y - r.w);
    vec2 hi = vec2(r.x + r.z, r.y);
    return step(lo.x, p.x) * step(p.x, hi.x) * step(lo.y, p.y) * step(p.y, hi.y);
}
";

/// A smear: a box sliding from the previous cursor to the current one; it blends `.rgb` only,
/// as most published smears do (the reason for the matte).
pub fn smear() -> String {
    format!(
        "{BOX}
void mainImage(out vec4 fragColor, in vec2 fragCoord) {{
    vec4 base = texture(iChannel0, fragCoord / iResolution.xy);
    float t = clamp((iTime - iTimeCursorChange) / 0.3, 0.0, 1.0);
    vec4 r = mix(iPreviousCursor, iCurrentCursor, t);
    float a = max(leon_box(fragCoord, r) * (1.0 - 0.5 * t), leon_box(fragCoord, iCurrentCursor));
    fragColor.rgb = mix(base.rgb, iCurrentCursorColor.rgb, a);
    fragColor.a = base.a;
}}
"
    )
}

/// A glow mixed over a colour brighter than 1.0, which the matte clamps.
pub const GLOW: &str = "
void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec4 base = texture(iChannel0, fragCoord / iResolution.xy);
    vec2 c = iCurrentCursor.xy + vec2(iCurrentCursor.z, -iCurrentCursor.w) * 0.5;
    float d = length(fragCoord - c);
    float a = exp(-d * d / 60.0);
    fragColor = vec4(mix(base.rgb, vec3(1.4, 0.6, 0.2), a), base.a);
}
";

/// A global initialized from a uniform: naga leaves it zero unless leon hoists it.
pub fn global() -> String {
    format!(
        "{BOX}
vec4 TRAIL_COLOR = vec4(iCurrentCursorColor.rgb, 1.0);
const float STRENGTH = 1.0;
void mainImage(out vec4 fragColor, in vec2 fragCoord) {{
    vec4 base = texture(iChannel0, fragCoord / iResolution.xy);
    fragColor = mix(base, TRAIL_COLOR, STRENGTH * leon_box(fragCoord, iCurrentCursor));
}}
"
    )
}

/// Reads the float[4] uniform `iChannelTime`, and the cursor that comes after it in the block.
pub fn float4() -> String {
    format!(
        "{BOX}
void mainImage(out vec4 fragColor, in vec2 fragCoord) {{
    vec4 base = texture(iChannel0, fragCoord / iResolution.xy);
    float a = leon_box(fragCoord, iCurrentCursor) + iChannelTime[2];
    fragColor = mix(base, vec4(0.9, 0.1, 0.2, 1.0), a);
}}
"
    )
}

/// Reads `iChannel0` through both names a shader uses for it.
pub fn channel() -> String {
    format!(
        "{BOX}
void mainImage(out vec4 fragColor, in vec2 fragCoord) {{
    vec2 uv = fragCoord / iResolution.xy;
    vec4 base = texture(iChannel0, uv);
    vec4 again = texture2D(iChannel0, uv);
    fragColor = mix(0.5 * (base + again), vec4(0.2, 0.8, 0.4, 1.0), 0.7 * leon_box(fragCoord, iCurrentCursor));
}}
"
    )
}

/// The cursor moved from the bottom left to (40, 30) 0.1 s ago, a block in amber.
pub fn moved() -> Uniforms {
    let mut u = Uniforms::default();
    u.set_size(W as f32, H as f32);
    u.time = 0.1;
    u.cursor_change_time = 0.0;
    u.current_cursor = [40., 30., 8., 14.];
    u.previous_cursor = [4., 44., 8., 14.];
    u.current_cursor_color = [1.0, 0.75, 0.2, 1.0];
    u.previous_cursor_color = u.current_cursor_color;
    u.cursor_visible = 1;
    u.focus = 1;
    u
}

/// A fake terminal picture, opaque BGRA8: a dark gradient with a light band of "text".
pub fn picture() -> Vec<u8> {
    let mut p = Vec::with_capacity((W * H * 4) as usize);
    for y in 0..H {
        for x in 0..W {
            let text = (y / 6) % 2 == 0 && x % 5 != 0;
            let (r, g, b) = if text { (200, 205, 190) } else { (20 + x as u8, 24, 30 + y as u8) };
            p.extend_from_slice(&[b, g, r, 255]);
        }
    }
    p
}

/// `src` drawn as the contract's host draws it: one pass over the picture.
pub fn contract(src: &str, u: &Uniforms) -> Vec<u8> {
    let p = compile(&translate(src, Mode::Contract).expect("translates")).expect("compiles");
    p.render((W, H), u, Some(&picture()))
}

/// `src` drawn by the overlay (the matte, no picture), as premultiplied BGRA8.
pub fn overlay(src: &str, u: &Uniforms) -> Vec<u8> {
    let p = compile(&translate(src, Mode::Matte).expect("translates")).expect("compiles");
    p.render((W, H), u, None)
}

/// The overlay composited over the picture, as Core Animation blends a premultiplied layer.
pub fn over_picture(o: &[u8]) -> Vec<u8> {
    let pic = picture();
    o.chunks(4)
        .zip(pic.chunks(4))
        .flat_map(|(o, p)| {
            let a = o[3] as f32 / 255.;
            let c = |i: usize| (o[i] as f32 + p[i] as f32 * (1. - a)).round().min(255.) as u8;
            [c(0), c(1), c(2), 255]
        })
        .collect()
}

/// The largest difference of any channel, and how many pixels differ by more than 3/255.
pub fn diff(a: &[u8], b: &[u8]) -> (u8, usize) {
    let max = a.iter().zip(b).map(|(x, y)| x.abs_diff(*y)).max().unwrap_or(0);
    let off = a.chunks(4).zip(b.chunks(4)).filter(|(x, y)| x.iter().zip(*y).any(|(p, q)| p.abs_diff(*q) > 3)).count();
    (max, off)
}

/// The BGRA8 pixel at (x, y).
pub fn at(img: &[u8], x: u64, y: u64) -> [u8; 4] {
    let i = ((y * W + x) * 4) as usize;
    [img[i], img[i + 1], img[i + 2], img[i + 3]]
}
