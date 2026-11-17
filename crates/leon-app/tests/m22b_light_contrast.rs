//! M22b criterion 6: every theme colour the app uses for text reads at 4.5:1 on every colour it
//! uses as a surface, in every built-in theme. The tokens come from scanning the views, so a view
//! that starts drawing a new pair is checked without anyone listing it here.

use std::collections::BTreeSet;

use leon_app::theme::{contrast, Theme, BUILTIN};

/// Backgrounds that never carry text, and why.
const NOT_SURFACES: [(&str, &str); 4] = [
    ("accent", "the focused pane's edge, a switch that is on, and the badge and hint labels checked in PAIRS"),
    ("working", "the host row's dot"),
    ("blocked", "the Activity count badge, checked in PAIRS"),
    ("border", "dividers, outlines and a switch that is off"),
];

/// Text colours that only ever sit on one background, and why.
const ONE_SURFACE: [(&str, &str); 1] = [("chrome", "the Activity count, on the blocked badge only")];

/// Text on a background other than the surfaces: (what, text, background).
const PAIRS: [(&str, &str, &str); 3] = [
    ("Activity count badge", "chrome", "blocked"),
    ("copy mode badge and hint labels", "bg", "accent"),
    ("pane text", "fg", "bg"),
];

/// Pairs known to be under 4.5:1 that stay, and why: (theme, text, background, why).
const KNOWN: [(&str, &str, &str, &str); 1] = [(
    "leon-dark",
    "working",
    "selected",
    "today's dark theme is kept pixel for pixel (M22b criterion 1); the 60% mark there has its dot too",
)];

/// Text colours that come from code other than `text_color(rgb(t.X))`: the status marks.
const STATUS: [&str; 4] = ["working", "blocked", "done", "error"];

fn token(t: &Theme, name: &str) -> u32 {
    match name {
        "fg" => t.fg,
        "bg" => t.bg,
        "cursor" => t.cursor,
        "selection" => t.selection,
        n => {
            let mut copy = *t;
            copy.slot(n).copied().unwrap_or_else(|| panic!("{n} isn't a theme token"))
        }
    }
}

/// The `t.NAME` tokens inside every `CALL(` on each line of the app's sources.
fn scan(call: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    for e in std::fs::read_dir(src).unwrap() {
        let text = std::fs::read_to_string(e.unwrap().path()).unwrap_or_default();
        for line in text.lines() {
            for (at, _) in line.match_indices(call) {
                // the call's own parentheses
                let rest = &line[at + call.len()..];
                let mut depth = 1;
                let end = rest
                    .char_indices()
                    .find(|(_, c)| {
                        depth += match c {
                            '(' => 1,
                            ')' => -1,
                            _ => 0,
                        };
                        depth == 0
                    })
                    .map_or(rest.len(), |(i, _)| i);
                for (i, _) in rest[..end].match_indices("t.") {
                    let before = rest[..i].chars().last();
                    if before.is_some_and(|c| c.is_alphanumeric() || c == '_') {
                        continue;
                    }
                    let name: String =
                        rest[i + 2..].chars().take_while(|c| c.is_ascii_lowercase() || *c == '_').collect();
                    out.insert(name);
                }
            }
        }
    }
    out
}

#[test]
fn the_scan_finds_the_views_colours() {
    let text = scan("text_color(");
    let bgs = scan(".bg(");
    for t in ["muted", "title", "text", "accent"] {
        assert!(text.contains(t), "no text in {t}? {text:?}");
    }
    for b in ["chrome", "hover", "selected", "bg"] {
        assert!(bgs.contains(b), "no {b} surface? {bgs:?}");
    }
}

#[test]
fn every_text_colour_reads_on_every_surface() {
    let skip = |n: &str| NOT_SURFACES.iter().any(|(s, _)| *s == n);
    let one = |n: &str| ONE_SURFACE.iter().any(|(s, _)| *s == n);
    let text: BTreeSet<String> = scan("text_color(").into_iter().chain(STATUS.map(String::from)).collect();
    let surfaces: Vec<String> = scan(".bg(").into_iter().filter(|b| !skip(b)).collect();
    let mut low = vec![];
    for (name, t) in BUILTIN.iter().map(|(n, t)| (*n, t)) {
        let mut check = |what: String, fg: &str, bg: &str| {
            let c = contrast(token(t, fg), token(t, bg));
            let known = KNOWN.iter().any(|(n, f, b, _)| (*n, *f, *b) == (name, fg, bg));
            if c < 4.5 && !known {
                low.push(format!(
                    "{name}: {what}: {fg} #{:06x} on {bg} #{:06x} is {c:.2}:1",
                    token(t, fg),
                    token(t, bg)
                ));
            }
        };
        for fg in text.iter().filter(|f| !one(f)) {
            for bg in &surfaces {
                check(format!("{fg} text on {bg}"), fg, bg);
            }
        }
        for (what, fg, bg) in PAIRS {
            check(what.to_string(), fg, bg);
        }
    }
    assert!(low.is_empty(), "under 4.5:1:\n{}", low.join("\n"));
}
