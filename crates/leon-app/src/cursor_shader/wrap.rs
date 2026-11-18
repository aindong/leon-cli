//! The contract a cursor shader is written against (docs/cursor-shaders.md): the user's file
//! defines `mainImage(out vec4, in vec2)` over the uniforms below; leon wraps it, translates the
//! result to MSL with naga, and reports an error at the user's own line.

use naga::back::msl;

/// What goes before the user's file: the uniform block in std140 order (as [`Uniforms`] lays it
/// out), `iChannel0`, the cursor style names. naga's GLSL reader takes no combined `sampler2D`
/// uniform, so `iChannel0` is a texture and a sampler joined by a macro.
pub const PREFIX: &str = r#"#version 450
layout(set = 0, binding = 1, std140) uniform Globals {
    vec3  iResolution;
    float iTime;
    float iTimeDelta;
    float iFrameRate;
    int   iFrame;
    vec3  iChannelResolution[4];
    vec4  iMouse;
    vec4  iDate;
    float iSampleRate;
    vec4  iCurrentCursor;
    vec4  iPreviousCursor;
    vec4  iCurrentCursorColor;
    vec4  iPreviousCursorColor;
    int   iCurrentCursorStyle;
    int   iPreviousCursorStyle;
    int   iCursorVisible;
    float iTimeCursorChange;
    float iTimeFocus;
    int   iFocus;
    vec3  iPalette[256];
    vec3  iBackgroundColor;
    vec3  iForegroundColor;
    vec3  iCursorColor;
    vec3  iCursorText;
    vec3  iSelectionForegroundColor;
    vec3  iSelectionBackgroundColor;
};
// outside the block: naga 25's MSL writer gives a float array in a uniform block a stride of 4
// bytes where std140 says 16, which moves every uniform after it (cursor_shader_naga.rs)
const float iChannelTime[4] = float[4](0.0, 0.0, 0.0, 0.0);
#define CURSORSTYLE_BLOCK        0
#define CURSORSTYLE_BLOCK_HOLLOW 1
#define CURSORSTYLE_BAR          2
#define CURSORSTYLE_UNDERLINE    3
#define CURSORSTYLE_LOCK         4
layout(set = 0, binding = 0) uniform texture2D leon_channel0;
layout(set = 0, binding = 2) uniform sampler leon_sampler0;
#define iChannel0 sampler2D(leon_channel0, leon_sampler0)
#define texture2D texture
layout(location = 0) out vec4 leon_frag_color;
"#;

/// What [`Mode::Matte`] adds after the prefix: every read of `iChannel0` is a flat base colour.
const MATTE: &str = r#"float leon_white;
vec4 leon_base() { return vec4(vec3(leon_white), 1.0); }
#define texture(s, uv) leon_base()
#define textureLod(s, uv, l) leon_base()
#define texelFetch(s, p, l) leon_base()
"#;

/// How the user's file is wrapped.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    /// As the contract's host runs it: one `mainImage` over the picture bound as `iChannel0`.
    Contract,
    /// The overlay's, which never sees the picture: `mainImage` runs over opaque black and over
    /// opaque white, and the two give the premultiplied colour and its coverage (difference
    /// matting). Exact for a shader that blends over the picture (`mix(base, c, a)`), whatever it
    /// does with alpha; wrong for one that reads it (`1 - texture(iChannel0, uv)`).
    Matte,
}

/// A wrapped file: the source naga reads, and for each of its lines the user's line it came from.
pub struct Wrapped {
    pub text: String,
    lines: Vec<Option<u32>>,
}

impl Wrapped {
    /// Text that is all leon's, no line of it the user's (tests).
    pub fn raw(text: String) -> Self {
        Wrapped { text, lines: Vec::new() }
    }

    /// The user's line for 1-based line `n` of the wrapped text, if it is one of theirs.
    pub fn user_line(&self, n: u32) -> Option<u32> {
        self.lines.get((n as usize).checked_sub(1)?).copied().flatten()
    }
}

/// `src` wrapped for `mode`. Each top-level variable with an initializer is declared bare where
/// it was and assigned at the start of `main` (see [`hoist_globals`]).
pub fn wrap(src: &str, mode: Mode) -> Wrapped {
    let (body, inits) = hoist_globals(src);
    let mut w = Wrapped { text: String::new(), lines: Vec::new() };
    let mut add = |text: &str, from: &mut dyn FnMut(usize) -> Option<u32>| {
        for (i, l) in text.lines().enumerate() {
            w.text.push_str(l);
            w.text.push('\n');
            w.lines.push(from(i));
        }
    };
    add(PREFIX, &mut |_| None);
    if mode == Mode::Matte {
        add(MATTE, &mut |_| None);
    }
    add(&body, &mut |i| Some(i as u32 + 1));
    // an assignment is the user's line; the conditional directives around it are leon's
    let init_lines: Vec<(Option<u32>, &str)> =
        inits.iter().flat_map(|(l, s)| s.lines().map(|t| ((!t.starts_with('#')).then_some(*l), t))).collect();
    let inits_text: String = init_lines.iter().map(|(_, t)| format!("    {t}\n")).collect();
    let mut init_line = |i: usize| init_lines.get(i).and_then(|(l, _)| *l);
    match mode {
        Mode::Contract => {
            add("void main() {", &mut |_| None);
            add(&inits_text, &mut init_line);
            add("    mainImage(leon_frag_color, gl_FragCoord.xy);\n}", &mut |_| None);
        }
        Mode::Matte => {
            add("void main() {\n    vec4 black;\n    vec4 white;\n    leon_white = 0.0;", &mut |_| None);
            add(&inits_text, &mut init_line);
            add("    mainImage(black, gl_FragCoord.xy);\n    leon_white = 1.0;", &mut |_| None);
            add(&inits_text, &mut init_line);
            add(
                "    mainImage(white, gl_FragCoord.xy);
    black = clamp(black, 0.0, 1.0);
    vec3 cover = clamp(vec3(1.0) - (clamp(white.rgb, 0.0, 1.0) - black.rgb), 0.0, 1.0);
    float a = max(cover.r, max(cover.g, cover.b));
    leon_frag_color = vec4(min(black.rgb, vec3(a)), a);
}",
                &mut |_| None,
            );
        }
    }
    w
}

/// The GLSL types a hoisted global can have.
const TYPES: &[&str] = &[
    "float", "int", "uint", "bool", "vec2", "vec3", "vec4", "ivec2", "ivec3", "ivec4", "uvec2", "uvec3", "uvec4",
    "bvec2", "bvec3", "bvec4", "mat2", "mat3", "mat4",
];

/// `src` with each top-level `TYPE NAME = EXPR;` (not `const`, one name, no array) turned into
/// `TYPE NAME;` padded to the lines it had, and the assignments `NAME = EXPR;` in order, each with
/// the user's line it starts on, inside the `#if` chains it sat in. naga 25 evaluates a global's non-constant initializer in the
/// entry point but never stores it, so the variable stays zero and the shader draws nothing.
/// shortcut: a scan of comments, braces, parentheses and `;`, not a GLSL parser; what it doesn't
/// recognize reaches naga unchanged. The hoist goes once naga stores such initializers
/// (cursor_shader_hoist_gpu.rs's second test fails then).
pub fn hoist_globals(src: &str) -> (String, Vec<(u32, String)>) {
    let mut out = String::with_capacity(src.len());
    let mut inits = Vec::new();
    let b = src.as_bytes();
    let (mut i, mut start, mut depth) = (0, 0, 0i32);
    // the top-level `#if…` chains the scan is inside, each as its directives so far
    let mut conds: Vec<Vec<String>> = Vec::new();
    // a statement's text with comments blanked, for deciding; `src` itself is what is kept
    let mut code = String::new();
    while i < b.len() {
        let rest = &src[i..];
        let line_start = i == 0 || b[i - 1] == b'\n';
        if depth == 0 && line_start && code.trim().is_empty() && rest.trim_start_matches([' ', '\t']).starts_with('#') {
            // a directive runs to a line that doesn't end in `\`
            let mut end = i;
            loop {
                end = src[end..].find('\n').map_or(b.len(), |n| end + n + 1);
                if end >= b.len() || !src[..end].trim_end_matches(['\n', '\r']).ends_with('\\') {
                    break;
                }
            }
            let directive = src[i..end].trim();
            let word = directive.trim_start_matches('#').trim_start();
            if word.starts_with("if") {
                conds.push(vec![directive.to_string()]);
            } else if word.starts_with("el") {
                if let Some(c) = conds.last_mut() {
                    c.push(directive.to_string());
                }
            } else if word.starts_with("endif") {
                conds.pop();
            }
            out.push_str(&src[start..end]);
            (i, start) = (end, end);
            code.clear();
            continue;
        }
        if rest.starts_with("//") {
            let end = rest.find('\n').map_or(b.len(), |n| i + n);
            i = end;
            continue;
        }
        if let Some(inside) = rest.strip_prefix("/*") {
            let end = inside.find("*/").map_or(b.len(), |n| i + 2 + n + 2);
            code.push_str(&"\n".repeat(src[i..end].matches('\n').count()));
            code.push(' ');
            i = end;
            continue;
        }
        let ch = rest.chars().next().unwrap_or(' ');
        code.push(ch);
        i += ch.len_utf8();
        let ended = match ch {
            '{' => {
                depth += 1;
                false
            }
            '}' => {
                depth -= 1;
                depth == 0
            }
            ';' => depth == 0,
            _ => false,
        };
        if ended {
            let stmt = &src[start..i];
            match hoistable(&code) {
                Some((ty, name, expr)) => {
                    let lead = leading_lines(&code);
                    let line = src[..start].matches('\n').count() as u32 + 1 + lead;
                    let after = stmt.matches('\n').count() as u32 - lead;
                    out.push_str(&format!("{}{ty} {name};{}", "\n".repeat(lead as usize), "\n".repeat(after as usize)));
                    let open: String = conds.iter().flatten().map(|d| format!("{d}\n")).collect();
                    let close = "#endif\n".repeat(conds.len());
                    inits.push((line, format!("{open}{name} = {expr};\n{close}").trim_end().to_string()));
                }
                None => out.push_str(stmt),
            }
            start = i;
            code.clear();
        }
    }
    out.push_str(&src[start..]);
    (out, inits)
}

/// The blank lines before a statement's first word.
fn leading_lines(code: &str) -> u32 {
    let first = code.find(|c: char| !c.is_whitespace()).unwrap_or(0);
    code[..first].matches('\n').count() as u32
}

/// `TYPE NAME = EXPR;` as its three parts, when it is one to hoist.
fn hoistable(code: &str) -> Option<(&str, &str, String)> {
    let code = code.trim().strip_suffix(';')?;
    let (decl, expr) = code.split_once('=')?;
    let mut words = decl.split_whitespace();
    let (ty, name) = (words.next()?, words.next()?);
    let ident = !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
    let expr = expr.split_whitespace().collect::<Vec<_>>().join(" ");
    // `a = 1, b = 2`: more than one declarator, left to naga
    let mut paren = 0i32;
    let several = expr.chars().any(|c| {
        paren += (c == '(') as i32 - (c == ')') as i32;
        c == ',' && paren == 0
    });
    (TYPES.contains(&ty) && ident && words.next().is_none() && !several && !expr.is_empty()).then_some((ty, name, expr))
}

/// A translation that failed: the reason, and the user's line where naga gave one in their file.
#[derive(Debug, Clone, PartialEq)]
pub struct Error {
    pub line: Option<u32>,
    pub message: String,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self.line {
            Some(l) => write!(f, "line {l}: {}", self.message),
            None => f.write_str(&self.message),
        }
    }
}

/// An error and its causes, one line.
fn chain(e: &dyn std::error::Error) -> String {
    let mut text = e.to_string();
    let mut source = e.source();
    while let Some(s) = source {
        text.push_str(": ");
        text.push_str(&s.to_string());
        source = s.source();
    }
    text
}

/// The user's file as MSL with leon's vertex function, ready for Metal; or why not.
pub fn translate(src: &str, mode: Mode) -> Result<String, Error> {
    msl(&wrap(src, mode))
}

/// A wrapped file as MSL with leon's vertex function; an error at the user's line where it is one.
pub fn msl(w: &Wrapped) -> Result<String, Error> {
    let at = |span: naga::Span| span.is_defined().then(|| w.user_line(span.location(&w.text).line_number)).flatten();
    let module = naga::front::glsl::Frontend::default()
        .parse(&naga::front::glsl::Options::from(naga::ShaderStage::Fragment), &w.text)
        .map_err(|e| {
            let first = e.errors.first();
            Error {
                line: first.and_then(|e| at(e.meta)),
                message: first.map_or_else(|| "the shader doesn't parse".into(), |e| e.kind.to_string()),
            }
        })?;
    let info = naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::all())
        .validate(&module)
        .map_err(|e| Error { line: e.spans().map(|(s, _)| *s).find_map(at), message: chain(e.as_inner()) })?;
    let rb = |binding| naga::ResourceBinding { group: 0, binding };
    let mut resources = msl::BindingMap::default();
    resources.insert(rb(0), msl::BindTarget { texture: Some(0), ..Default::default() });
    resources.insert(rb(1), msl::BindTarget { buffer: Some(0), ..Default::default() });
    resources.insert(rb(2), msl::BindTarget { sampler: Some(msl::BindSamplerTarget::Inline(0)), ..Default::default() });
    let mut options = msl::Options { lang_version: (2, 4), ..Default::default() };
    options.per_entry_point_map.insert("main".into(), msl::EntryPointResources { resources, ..Default::default() });
    options.inline_samplers.push(msl::sampler::InlineSampler {
        coord: msl::sampler::Coord::Normalized,
        address: [msl::sampler::Address::ClampToEdge; 3],
        border_color: msl::sampler::BorderColor::TransparentBlack,
        mag_filter: msl::sampler::Filter::Linear,
        min_filter: msl::sampler::Filter::Linear,
        mip_filter: None,
        lod_clamp: None,
        max_anisotropy: None,
        compare_func: msl::sampler::CompareFunc::Never,
    });
    let msl_err = |e: msl::Error| Error { line: None, message: format!("translating to Metal: {e}") };
    let (out, ti) = msl::write_string(&module, &info, &options, &msl::PipelineOptions::default()).map_err(msl_err)?;
    let name = match ti.entry_point_names.into_iter().next() {
        Some(Ok(name)) => name,
        Some(Err(e)) => return Err(Error { line: None, message: format!("translating to Metal: {e}") }),
        None => return Err(Error { line: None, message: "no entry point".into() }),
    };
    Ok(format!("{out}\n{}", VERTEX.replace("FRAGMENT_NAME", &name)))
}

/// A triangle over the whole target; the fragment's name is filled in, so both are found in one
/// library ([`fragment_name`]).
const VERTEX: &str = r#"
struct leon_vertex_out { metal::float4 position [[position]]; };
vertex leon_vertex_out leon_vertex(uint vid [[vertex_id]]) {
    metal::float2 p = metal::float2((vid << 1) & 2, vid & 2);
    return leon_vertex_out { metal::float4(p * 2.0 - 1.0, 0.0, 1.0) };
}
// fragment: FRAGMENT_NAME
"#;

/// The fragment function's name in MSL from [`translate`].
pub fn fragment_name(msl: &str) -> &str {
    msl.rsplit("// fragment: ").next().unwrap_or("main_").trim()
}

/// The uniform block as std140 lays it out (offsets in the comments), vec3s padded to 16 bytes.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Uniforms {
    pub resolution: [f32; 3], // 0
    pub time: f32,            // 12
    pub time_delta: f32,
    pub frame_rate: f32,
    pub frame: i32,
    _p0: f32,
    pub channel_resolution: [[f32; 4]; 4], // 32
    pub mouse: [f32; 4],                   // 96
    pub date: [f32; 4],
    pub sample_rate: f32, // 128
    _p1: [f32; 3],
    pub current_cursor: [f32; 4], // 144
    pub previous_cursor: [f32; 4],
    pub current_cursor_color: [f32; 4],
    pub previous_cursor_color: [f32; 4],
    pub current_cursor_style: i32, // 208
    pub previous_cursor_style: i32,
    pub cursor_visible: i32,
    pub cursor_change_time: f32,
    pub time_focus: f32,
    pub focus: i32,
    _p2: [f32; 2],
    pub palette: [[f32; 4]; 256],   // 240
    pub background_color: [f32; 4], // 4336
    pub foreground_color: [f32; 4],
    pub cursor_color: [f32; 4],
    pub cursor_text: [f32; 4],
    pub selection_foreground_color: [f32; 4],
    pub selection_background_color: [f32; 4],
}

const _: () = {
    assert!(std::mem::offset_of!(Uniforms, sample_rate) == 128);
    assert!(std::mem::offset_of!(Uniforms, current_cursor) == 144);
    assert!(std::mem::offset_of!(Uniforms, current_cursor_style) == 208);
    assert!(std::mem::offset_of!(Uniforms, palette) == 240);
    assert!(std::mem::offset_of!(Uniforms, background_color) == 4336);
    assert!(std::mem::size_of::<Uniforms>() == 4432);
};

impl Default for Uniforms {
    fn default() -> Self {
        // SAFETY: plain f32/i32 fields; all zeros is a valid value of each
        unsafe { std::mem::zeroed() }
    }
}

impl Uniforms {
    /// The target is `w`×`h` device pixels: `iResolution` and `iChannelResolution[0]`.
    pub fn set_size(&mut self, w: f32, h: f32) {
        self.resolution = [w, h, 1.];
        self.channel_resolution[0] = [w, h, 1., 0.];
    }

    /// Its bytes, as the uniform buffer takes them.
    pub fn bytes(&self) -> &[u8] {
        // SAFETY: repr(C), plain f32/i32 fields and explicit padding: every byte is initialized
        unsafe { std::slice::from_raw_parts((self as *const Self).cast(), std::mem::size_of::<Self>()) }
    }
}

/// `rgb` (0xRRGGBB) as a 0..1 vec4 with alpha 1.
pub fn color(rgb: u32) -> [f32; 4] {
    let c = |s: u32| ((rgb >> s) & 0xff) as f32 / 255.;
    [c(16), c(8), c(0), 1.]
}
