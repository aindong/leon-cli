//! The web page's QR code (qr.rs) decodes back to its URL, drawn as modules (Settings) and as
//! half blocks (`leon server status --qr`), and its colours scan in every built-in theme.
use leon_app::qr::{colours, half_blocks, modules, runs, terminal_lines, MIN_CONTRAST, QUIET};

const URLS: [&str; 3] = [
    "http://devbox.example-tailnet.ts.net:7297/",
    "https://leon-demo.example-tailnet.ts.net:7297/",
    "http://[fd7a:115c:a1e0::1234]:7297/",
];

/// Decode a picture `side` pixels square in which `dark(x, y)` is a dark pixel.
fn decode(side: usize, dark: impl Fn(usize, usize) -> bool) -> String {
    let mut img = rqrr::PreparedImage::prepare_from_bitmap(side, side, dark);
    let grids = img.detect_grids();
    assert_eq!(grids.len(), 1, "one code found");
    grids[0].decode().expect("it decodes").1
}

/// The window paints `runs` and nothing else on a light field: rebuilt from them at 4 px a
/// module with the quiet zone, the picture decodes.
#[test]
fn the_painted_runs_decode_to_the_url() {
    for url in URLS {
        let m = modules(url).unwrap();
        assert!(m.iter().all(|row| row.len() == m.len()), "square");
        let r = runs(&m);
        let dark = |mx: usize, my: usize| {
            mx >= QUIET
                && my >= QUIET
                && r.iter().any(|&(y, x, len)| my - QUIET == y && (x..x + len).contains(&(mx - QUIET)))
        };
        let got = decode((m.len() + 2 * QUIET) * 4, |x, y| dark(x / 4, y / 4));
        assert_eq!(got, url);
        let painted: usize = r.iter().map(|&(_, _, len)| len).sum();
        assert_eq!(painted, m.iter().flatten().filter(|d| **d).count(), "every dark module once");
    }
}

#[test]
fn the_half_blocks_decode_to_the_url() {
    for url in URLS {
        let lines = half_blocks(url).unwrap();
        // each character is one module wide and two high; drawn (light) or blank (dark) per half
        let rows: Vec<Vec<char>> = lines.iter().map(|l| l.chars().collect()).collect();
        let side = rows[0].len();
        assert!(rows.iter().all(|r| r.len() == side), "every line as wide");
        assert_eq!(rows.len(), side.div_ceil(2));
        let light = |x: usize, y: usize| match rows[y / 2][x] {
            '█' => true,
            '▀' => y.is_multiple_of(2),
            '▄' => !y.is_multiple_of(2),
            ' ' => false,
            c => panic!("unexpected {c:?}"),
        };
        let got = decode(side * 4, |x, y| !light(x / 4, y / 4));
        assert_eq!(got, url);
        assert!(rows[0].iter().all(|c| *c == '█'), "a light quiet zone on top");
    }
}

/// On a terminal every line is white (the blocks, light) on black (the blanks, dark); piped, the
/// same characters bare.
#[test]
fn terminal_lines_pin_the_colours() {
    let url = URLS[0];
    let plain = half_blocks(url).unwrap();
    assert_eq!(terminal_lines(url, false).unwrap(), plain);
    let coloured = terminal_lines(url, true).unwrap();
    assert_eq!(coloured.len(), plain.len());
    for (c, p) in coloured.iter().zip(&plain) {
        assert_eq!(c, &format!("\x1b[97;40m{p}\x1b[0m"));
    }
}

#[test]
fn too_long_a_text_has_no_code() {
    assert!(modules(&"x".repeat(8000)).is_none());
    assert!(half_blocks(&"x".repeat(8000)).is_none());
}

#[test]
fn the_colours_are_dark_on_light_with_contrast_in_every_theme() {
    for (name, t) in leon_core::theme::BUILTIN {
        let (field, module) = colours(t.bg, t.title);
        let c = leon_core::theme::contrast(field, module);
        assert!(c >= MIN_CONTRAST, "{name}: {c}");
        assert!(
            leon_core::theme::contrast(field, 0) > leon_core::theme::contrast(module, 0),
            "{name}: the field is the lighter one"
        );
        assert!((field, module) == (t.bg, t.title) || (field, module) == (t.title, t.bg), "{name}: its own colours");
    }
    let (light, dark) = (leon_core::theme::LIGHT.bg, leon_core::theme::DARK.bg);
    assert_eq!(colours(0x777777, 0x888888), (light, dark), "too close: Parchment and Coal");
    assert!(leon_core::theme::contrast(light, dark) >= MIN_CONTRAST);
}
