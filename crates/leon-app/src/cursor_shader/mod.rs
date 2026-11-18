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
pub use overlay::{attach, configure, detach, enable, feed, fit, state};
#[cfg(feature = "gui")]
pub use program::{compile, texture, Program};
pub use wrap::{color, fragment_name, hoist_globals, msl, translate, wrap, Error, Mode, Uniforms, Wrapped, PREFIX};

use std::path::Path;

/// The largest shader file read: a cursor shader is a page or two of GLSL.
pub const MAX_BYTES: u64 = 256 * 1024;

/// The file at `path`, read whole if it is a small enough text file.
pub fn read(path: &Path) -> Result<String, String> {
    let shown = path.display();
    let meta = std::fs::metadata(path).map_err(|e| format!("{shown}: {e}"))?;
    if !meta.is_file() {
        return Err(format!("{shown}: not a file"));
    }
    if meta.len() > MAX_BYTES {
        return Err(format!("{shown}: {} bytes, more than a cursor shader's {MAX_BYTES}", meta.len()));
    }
    std::fs::read_to_string(path).map_err(|e| format!("{shown}: {e}"))
}

/// The file at `path` translated as the overlay runs it: MSL, or the reason it fails as
/// `NAME:LINE: why` (the line in the user's file, where naga gives one).
pub fn load(path: &Path) -> Result<String, String> {
    let src = read(path)?;
    translate(&src, Mode::Matte).map_err(|e| {
        let name = path.file_name().map_or_else(|| path.display().to_string(), |n| n.to_string_lossy().into());
        match e.line {
            Some(l) => format!("{name}:{l}: {}", e.message),
            None => format!("{name}: {}", e.message),
        }
    })
}
