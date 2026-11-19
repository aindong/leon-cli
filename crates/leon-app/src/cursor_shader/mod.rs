//! Shadertoy-style cursor shaders (docs/cursor-shaders.md, docs/architecture/cursor-shaders.md).
//! A user's GLSL file is wrapped and translated to MSL with naga (`wrap`), compiled at runtime
//! with Metal (`program`), and drawn by an overlay of leon's own: a transparent `CAMetalLayer`
//! above GPUI's, with a display link that runs only while a cursor change animates (`overlay`),
//! fed the cursor the focused pane's grid just painted (`cursor`). The overlay never sees GPUI's
//! picture, so a shader that reworks it (CRT, bloom, invert) doesn't work.

mod cursor;
#[cfg(feature = "gui")]
mod overlay;
#[cfg(feature = "gui")]
mod program;
mod wrap;

pub use cursor::{Cursor, Drawn, Next, Style};
#[cfg(feature = "gui")]
pub use overlay::Overlay;
#[cfg(feature = "gui")]
pub use program::{compile, compiles, texture, Program};
pub use wrap::{color, fragment_name, hoist_globals, msl, translate, wrap, Error, Mode, Uniforms, Wrapped, PREFIX};

use std::path::Path;

/// The largest shader file read: a cursor shader is a page or two of GLSL.
pub const MAX_BYTES: u64 = 256 * 1024;

/// The file at `path`, read whole if it is a small enough text file (opened once, and read no
/// further than the limit, whatever it grows to meanwhile).
pub fn read(path: &Path) -> Result<String, String> {
    use std::io::Read;
    let shown = path.display();
    let file = std::fs::File::open(path).map_err(|e| format!("{shown}: {e}"))?;
    if !file.metadata().is_ok_and(|m| m.is_file()) {
        return Err(format!("{shown}: not a file"));
    }
    let mut bytes = Vec::new();
    file.take(MAX_BYTES + 1).read_to_end(&mut bytes).map_err(|e| format!("{shown}: {e}"))?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err(format!("{shown}: more than a cursor shader's {MAX_BYTES} bytes"));
    }
    String::from_utf8(bytes).map_err(|_| format!("{shown}: not UTF-8 text"))
}

/// The file at `path` translated as the overlay runs it: MSL, or the reason it fails as
/// `NAME:LINE: why` (the line in the user's file, where naga gives one).
pub fn load(path: &Path) -> Result<String, String> {
    let src = read(path)?;
    let name = path.file_name().map_or_else(|| path.display().to_string(), |n| n.to_string_lossy().into());
    // naga's GLSL reader is the user's file's first reader: a panic in it is that file's error
    let translated = std::panic::catch_unwind(|| translate(&src, Mode::Matte))
        .map_err(|_| format!("{name}: the translator failed on this file"))?;
    translated.map_err(|e| match e.line {
        Some(l) => format!("{name}:{l}: {}", e.message),
        None => format!("{name}: {}", e.message),
    })
}
