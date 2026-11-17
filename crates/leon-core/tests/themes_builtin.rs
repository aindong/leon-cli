//! Themes criteria 1–2 (.agent/plans/2026-11-17-themes.md): leon ships eight built-in themes, three
//! dark and three light besides leon-dark and leon-light, and every one of them reads: body text,
//! the app's text on its surfaces, the ANSI colours, selection and find marks, and status colours
//! that can be told apart. `cargo test -p leon-core --test themes_builtin -- --nocapture` prints
//! each theme's worst pairs.

use leon_core::theme::{builtin, contrast, mix, Theme, BUILTIN};

const NEW: [(&str, bool); 6] = [
    ("leon-maroon", true),
    ("leon-tide", true),
    ("leon-coal", true),
    ("leon-linen", false),
    ("leon-sand", false),
    ("leon-mist", false),
];

/// The default fade of the panes without the focus (leon-lua's `FADE`, which leon-core can't see).
const FADE: f32 = 0.35;

/// CIE L*a*b* of an sRGB colour (D65).
fn lab(c: u32) -> [f64; 3] {
    let lin = |s: u32| {
        let v = (c >> s & 0xff) as f64 / 255.;
        if v <= 0.04045 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    let (r, g, b) = (lin(16), lin(8), lin(0));
    let x = (0.4124 * r + 0.3576 * g + 0.1805 * b) / 0.95047;
    let y = 0.2126 * r + 0.7152 * g + 0.0722 * b;
    let z = (0.0193 * r + 0.1192 * g + 0.9505 * b) / 1.08883;
    let f = |t: f64| if t > 0.008856 { t.cbrt() } else { 7.787 * t + 16. / 116. };
    [116. * f(y) - 16., 500. * (f(x) - f(y)), 200. * (f(y) - f(z))]
}

/// How far apart two colours look (CIE76 ΔE; about 2 is the smallest difference people see).
fn delta_e(a: u32, b: u32) -> f64 {
    let (a, b) = (lab(a), lab(b));
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

#[test]
fn eight_themes_three_new_dark_and_three_new_light() {
    let names: Vec<&str> = BUILTIN.iter().map(|(n, _)| *n).collect();
    assert_eq!(&names[..2], ["leon-dark", "leon-light"], "the first two stay first");
    for (name, dark) in NEW {
        let t = builtin(name).unwrap_or_else(|| panic!("{name} is built in"));
        assert_eq!(t.is_dark(), dark, "{name} is {}", if dark { "dark" } else { "light" });
    }
    assert_eq!(BUILTIN.len(), 8, "{names:?}");
    let mut unique = names.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(unique.len(), names.len(), "names are unique: {names:?}");
    let colours: Vec<Theme> = BUILTIN.iter().map(|(_, t)| *t).collect();
    for (i, a) in colours.iter().enumerate() {
        assert!(colours[i + 1..].iter().all(|b| b != a), "{} is a copy of another theme", names[i]);
    }
}

/// Every rule a theme must meet, as "theme: what is N" lines for the ones it misses; prints the
/// worst body, ANSI and status numbers.
fn problems(name: &str, t: &Theme) -> Vec<String> {
    let mut out = vec![];
    let mut need = |what: String, fg: u32, bg: u32, min: f64| {
        let c = contrast(fg, bg);
        if c < min {
            out.push(format!("{name}: {what} is {c:.2}:1 ({fg:06X} on {bg:06X}), under {min}:1"));
        }
        c
    };
    let body = need("fg on bg".into(), t.fg, t.bg, 4.5);
    need("fg on selection".into(), t.fg, t.selection, 4.5);
    need("link on bg".into(), t.link, t.bg, 4.5);
    let texts = [
        ("text", t.text),
        ("title", t.title),
        ("muted", t.muted),
        ("accent", t.accent),
        ("working", t.working),
        ("blocked", t.blocked),
        ("done", t.done),
        ("error", t.error),
    ];
    let surfaces = [("bg", t.bg), ("chrome", t.chrome), ("hover", t.hover), ("selected", t.selected)];
    let mut ui = f64::MAX;
    for (f, fg) in texts {
        for (b, bg) in surfaces {
            // leon-dark is kept pixel for pixel (m22a); the app's scan test lists the same pair
            if (name, f, b) == ("leon-dark", "working", "selected") {
                continue;
            }
            ui = ui.min(need(format!("{f} on {b}"), fg, bg, 4.5));
        }
    }
    need("chrome on blocked (the Activity badge)".into(), t.chrome, t.blocked, 4.5);
    need("bg on accent (copy mode and hint labels)".into(), t.bg, t.accent, 4.5);
    let mut ansi = (f64::MAX, 0);
    for i in (1..=6).chain(9..=14) {
        let c = need(format!("ANSI {i} on bg"), t.ansi[i], t.bg, 4.5);
        if c < ansi.0 {
            ansi = (c, i);
        }
    }
    need("ANSI 8 (dim text) on bg".into(), t.ansi[8], t.bg, 2.5);
    need("accent (the current match's outline) on bg".into(), t.accent, t.bg, 3.);
    let faded = (mix(t.fg, t.chrome, FADE), mix(t.bg, t.chrome, FADE));
    need("fg on bg in a faded pane".into(), faded.0, faded.1, 4.5);
    let mut apart = |what: &str, a: u32, b: u32, min: f64| {
        let d = delta_e(a, b);
        if d < min {
            out.push(format!("{name}: {what} are ΔE {d:.1} apart, under {min}"));
        }
        d
    };
    apart("selection and bg", t.selection, t.bg, 10.);
    apart("find marks and bg", t.found, t.bg, 15.);
    apart("find marks and selection", t.found, t.selection, 10.);
    let states = [("working", t.working), ("done", t.done), ("blocked", t.blocked), ("error", t.error)];
    let mut status = f64::MAX;
    for (i, (a, ca)) in states.iter().enumerate() {
        for (b, cb) in &states[i + 1..] {
            status = status.min(apart(&format!("{a} and {b}"), *ca, *cb, 25.));
        }
    }
    println!(
        "{name:12} body {body:5.2}:1  UI text {ui:5.2}:1  worst ANSI {} {:5.2}:1  status ΔE ≥ {status:4.1}",
        ansi.1, ansi.0
    );
    out
}

#[test]
fn every_built_in_theme_reads() {
    let all: Vec<String> = BUILTIN.iter().flat_map(|(n, t)| problems(n, t)).collect();
    assert!(all.is_empty(), "{}", all.join("\n"));
}
