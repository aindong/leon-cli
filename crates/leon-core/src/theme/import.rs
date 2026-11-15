//! Theme files to [`Theme`]s: Ghostty's `key = value` syntax, which leon also stores its own themes
//! in (with the extra keys in [`Theme::TOKENS`]), and iTerm2's `.itermcolors` XML plists. A file
//! only has to give a background and a foreground; the app's chrome is mixed from those, and the
//! status colours and any missing ANSI colour come from leon's dark or light theme, whichever
//! matches the background.

use super::{mix, Theme, DARK, LIGHT};

/// The biggest theme file read; real ones are a few KB.
pub const MAX_BYTES: usize = 1 << 20;

/// The keys a Ghostty theme may set that leon has no colour for.
const IGNORED: [&str; 3] = ["cursor-text", "selection-foreground", "bold-color"];

/// The named colours a file wrote, by theme-file key, and its ANSI colours.
#[derive(Default)]
struct Given {
    named: Vec<(String, u32)>,
    ansi: [Option<u32>; 16],
}

impl Given {
    fn get(&self, key: &str) -> Option<u32> {
        self.named.iter().rev().find(|(k, _)| k == key).map(|(_, c)| *c)
    }
}

/// A theme file's bytes, at most [`MAX_BYTES`] + 1 of them (so [`parse`] can refuse a bigger one),
/// and only from a regular file: a FIFO named `x.theme` would block the server's config load.
pub fn read_file(path: &std::path::Path) -> std::io::Result<Vec<u8>> {
    use std::io::Read;
    if !std::fs::metadata(path)?.is_file() {
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, "not a regular file"));
    }
    let mut bytes = vec![];
    std::fs::File::open(path)?.take(MAX_BYTES as u64 + 1).read_to_end(&mut bytes)?;
    Ok(bytes)
}

/// Read a theme file of either kind. Errors say what is wrong and, for text files, on which line.
pub fn parse(bytes: &[u8]) -> Result<Theme, String> {
    if bytes.len() > MAX_BYTES {
        return Err("the file is over 1 MiB; a theme file is a few KB".into());
    }
    if bytes.starts_with(b"bplist") {
        return Err("a binary plist: convert it to XML first (plutil -convert xml1 FILE)".into());
    }
    let text = std::str::from_utf8(bytes).map_err(|_| "not a text file (it isn't UTF-8)".to_string())?;
    let text = text.trim_start_matches('\u{feff}');
    let given = if text.trim_start().starts_with('<') { iterm(text)? } else { ghostty(text)? };
    finish(&given)
}

/// The theme as a leon theme file, which [`parse`] reads back to the same theme.
pub fn to_text(t: &Theme) -> String {
    let mut out = String::from("# A leon theme: Ghostty's syntax, plus leon's own colours below the palette.\n");
    let mut t = *t;
    for key in ["background", "foreground", "cursor-color", "selection-background"] {
        out += &format!("{key} = #{:06x}\n", t.slot(key).map_or(0, |c| *c));
    }
    for (i, c) in t.ansi.iter().enumerate() {
        out += &format!("palette = {i}=#{c:06x}\n");
    }
    for key in Theme::TOKENS {
        out += &format!("{key} = #{:06x}\n", t.slot(key).map_or(0, |c| *c));
    }
    out
}

/// `s` for an error message: a junk line can be a megabyte long, and a control character (an OSC
/// 52 clipboard write) must not reach the terminal raw.
fn clip(s: &str) -> String {
    let mut out: String =
        s.chars().take(40).map(|c| if c.is_control() { c.escape_default().collect() } else { c.to_string() }).collect();
    if s.chars().nth(40).is_some() {
        out.push('…');
    }
    out
}

/// `#rrggbb` or `rrggbb`.
fn hex(s: &str) -> Option<u32> {
    let digits = s.strip_prefix('#').unwrap_or(s);
    (digits.len() == 6 && digits.bytes().all(|b| b.is_ascii_hexdigit()))
        .then(|| u32::from_str_radix(digits, 16).ok())
        .flatten()
}

fn ghostty(text: &str) -> Result<Given, String> {
    let (mut g, mut probe) = (Given::default(), DARK);
    for (n, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let at = |m: String| format!("line {}: {m}", n + 1);
        let colour = |v: &str| hex(v).ok_or_else(|| at(format!("{:?} is not a #rrggbb colour", clip(v))));
        let (key, value) = line.split_once('=').ok_or_else(|| at(format!("{:?} is not `key = value`", clip(line))))?;
        let (key, value) = (key.trim(), value.trim());
        if key == "palette" {
            let (i, c) =
                value.split_once('=').ok_or_else(|| at(format!("palette {:?} is not N=#rrggbb", clip(value))))?;
            let i: usize = i
                .trim()
                .parse()
                .ok()
                .filter(|i| *i < 256)
                .ok_or_else(|| at(format!("palette index {:?} is not a number from 0 to 255", clip(i.trim()))))?;
            let c = colour(c.trim())?;
            if let Some(slot) = g.ansi.get_mut(i) {
                *slot = Some(c); // 16 to 255 are valid Ghostty but leon computes those (theme::indexed)
            }
        } else if IGNORED.contains(&key) {
            colour(value)?;
        } else if probe.slot(key).is_some() {
            g.named.push((key.to_string(), colour(value)?));
        } else {
            return Err(at(format!(
                "{} is not a theme colour (a theme sets background, foreground, cursor-color, \
                 selection-background and palette)",
                clip(key)
            )));
        }
    }
    Ok(g)
}

fn iterm(text: &str) -> Result<Given, String> {
    // The DTD line every .itermcolors has needs allow_dtd (roxmltree never fetches it). Entities
    // are refused outright: roxmltree bounds their nesting, not how often a file uses one, so a
    // 1 MiB file could expand to gigabytes. With none, the parse is linear and the node limit holds.
    if text.contains("<!ENTITY") {
        return Err("not an iTerm2 colour file: it declares XML entities, which those never do".into());
    }
    let opt = roxmltree::ParsingOptions { allow_dtd: true, nodes_limit: 100_000 };
    let doc = roxmltree::Document::parse_with_options(text, opt).map_err(|e| format!("not a valid XML plist: {e}"))?;
    let root = doc.root_element();
    if !root.has_tag_name("plist") {
        return Err(format!(
            "not an iTerm2 colour file: the root element is <{}>, not <plist>",
            clip(root.tag_name().name())
        ));
    }
    let dict = root.children().find(|n| n.is_element()).filter(|n| n.has_tag_name("dict"));
    let dict = dict.ok_or("not an iTerm2 colour file: the plist's top level isn't a <dict> of colours")?;
    let mut g = Given::default();
    let mut items = dict.children().filter(|n| n.is_element());
    while let Some(k) = items.next() {
        if !k.has_tag_name("key") {
            return Err(format!("expected a <key> in the colour <dict>, found <{}>", clip(k.tag_name().name())));
        }
        let name = &clip(k.text().unwrap_or_default());
        let value = items.next().ok_or_else(|| format!("{name} has no value"))?;
        let key = match name.as_str() {
            "Background Color" => "background",
            "Foreground Color" => "foreground",
            "Cursor Color" => "cursor-color",
            "Selection Color" => "selection-background",
            _ => match name.strip_prefix("Ansi ").and_then(|n| n.strip_suffix(" Color")) {
                Some(i) => {
                    let i = i.parse::<usize>().ok().filter(|i| *i < 16);
                    let i = i.ok_or_else(|| format!("{name} is not an ANSI colour (they are 0 to 15)"))?;
                    g.ansi[i] = Some(component_dict(value).map_err(|e| format!("{name}: {e}"))?);
                    continue;
                }
                None => continue, // Bold Color, Link Color, Badge Color…: nothing in leon draws them
            },
        };
        g.named.push((key.into(), component_dict(value).map_err(|e| format!("{name}: {e}"))?));
    }
    for (key, name) in [("background", "Background Color"), ("foreground", "Foreground Color")] {
        if g.get(key).is_none() {
            return Err(format!("no {name}: this isn't an iTerm2 colour file"));
        }
    }
    Ok(g)
}

/// An iTerm2 colour: a <dict> of `Red/Green/Blue Component` reals from 0 to 1.
fn component_dict(d: roxmltree::Node) -> Result<u32, String> {
    if !d.has_tag_name("dict") {
        return Err(format!("expected a colour <dict>, found <{}>", clip(d.tag_name().name())));
    }
    let els: Vec<_> = d.children().filter(|n| n.is_element()).collect();
    let mut rgb = 0;
    for (shift, want) in [(16, "Red Component"), (8, "Green Component"), (0, "Blue Component")] {
        let at = els.iter().position(|n| n.has_tag_name("key") && n.text() == Some(want));
        let v = at.and_then(|i| els.get(i + 1)).ok_or_else(|| format!("no {want}"))?;
        let x: f64 = v.text().unwrap_or_default().trim().parse().map_err(|_| format!("{want} is not a number"))?;
        // a wide-gamut export can land a hair outside 0..1; further out is not a colour
        if !(-0.001..=1.001).contains(&x) {
            return Err(format!("{want} {x} is not from 0 to 1"));
        }
        let x = x.clamp(0., 1.);
        // shortcut: every colour space is read as sRGB; P3 colours come out a little duller.
        rgb |= ((x * 255.).round() as u32) << shift;
    }
    Ok(rgb)
}

fn finish(g: &Given) -> Result<Theme, String> {
    let bg = g.get("background").ok_or("no background colour: this isn't a theme file")?;
    let fg = g.get("foreground").ok_or("no foreground colour: this isn't a theme file")?;
    let probe = Theme { bg, ..DARK };
    let mut t = if probe.is_dark() { DARK } else { LIGHT };
    t.bg = bg;
    t.fg = fg;
    for (slot, given) in t.ansi.iter_mut().zip(g.ansi) {
        *slot = given.unwrap_or(*slot);
    }
    t.cursor = g.get("cursor-color").unwrap_or(fg);
    t.selection = g.get("selection-background").unwrap_or(mix(bg, fg, 0.25));
    // The chrome sits a step off the pane, as Iron does off Coal.
    t.chrome = mix(bg, fg, 0.06);
    t.hover = mix(bg, fg, 0.12);
    t.border = mix(bg, fg, 0.16);
    (t.text, t.title, t.muted) = (fg, fg, mix(fg, t.chrome, 0.3));
    (t.accent, t.selected) = (t.cursor, t.selection);
    t.found = mix(bg, t.ansi[3], 0.35);
    t.link = t.ansi[4];
    for (key, c) in &g.named {
        if let Some(slot) = t.slot(key) {
            *slot = *c;
        }
    }
    Ok(t)
}
