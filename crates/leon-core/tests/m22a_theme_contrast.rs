//! M22a criterion 1 (.agent/plans/2026-11-15-m22a-themes-settings.md): leon's dark theme is today's
//! colours, the light one is built on Parchment, and both keep text and status colours at WCAG AA.

use leon_core::theme::{builtin, contrast, Theme, DARK, LIGHT};

/// Today's app colours, copied from crates/leon-core/src/theme.rs before M22a: M22b swaps the app
/// to the model, and that swap must change nothing.
const TODAY: Theme = Theme {
    fg: 0xD8CEC2,
    bg: 0x1F1C19,
    cursor: 0xF3A747,
    ansi: [
        0x3A342E, 0xE0715F, 0xA9B98F, 0xE8C77D, 0x86A6C2, 0xC994B0, 0x8FBCB0, 0xD8CEC2, // normal
        0x6E645A, 0xF08A7A, 0xC2D1A8, 0xF3D79A, 0xA2BDD6, 0xDDB0C8, 0xA9D3C8, 0xF5F0E7, // bright
    ],
    chrome: 0x292521,
    hover: 0x35312D,
    border: 0x3D3934,
    text: 0xD8CEC2,
    title: 0xF5F0E7,
    muted: 0xACA49A,
    accent: 0xF3A747,
    selected: 0x542D35,
    brand: 0xB64C32,
    working: 0xE78155,
    done: 0xB8C6AC,
    blocked: 0xE8C77D,
    error: 0xE28E9B,
    selection: 0x542D35,
    found: 0x5B4C30,
    link: 0xE6AA91,
};

#[test]
fn dark_is_todays_app_and_light_is_parchment() {
    assert_eq!(DARK, TODAY, "every token of the dark theme is today's");
    assert_eq!((LIGHT.bg, LIGHT.fg, LIGHT.text), (0xF5F0E7, 0x292521, 0x292521), "Parchment under Iron");
    assert_eq!(builtin("leon-dark"), Some(DARK));
    assert_eq!(builtin("leon-light"), Some(LIGHT));
    assert_eq!(builtin("nope"), None);
    assert!(DARK.is_dark() && !LIGHT.is_dark());
}

fn check(name: &str, t: &Theme, problems: &mut Vec<String>) {
    let mut pairs = vec![
        ("fg on bg", t.fg, t.bg),
        ("text on chrome", t.text, t.chrome),
        ("title on chrome", t.title, t.chrome),
        ("muted on chrome", t.muted, t.chrome),
        ("text on hover", t.text, t.hover),
        ("muted on hover", t.muted, t.hover),
        ("title on selected", t.title, t.selected),
        ("muted on selected", t.muted, t.selected),
        ("fg on selection", t.fg, t.selection),
        ("link on bg", t.link, t.bg),
    ];
    for (state, c) in [("working", t.working), ("blocked", t.blocked), ("done", t.done), ("error", t.error)] {
        pairs.push((state, c, t.chrome));
        pairs.push((state, c, t.bg));
    }
    for (what, fg, bg) in pairs {
        let c = contrast(fg, bg);
        if c < 4.5 {
            problems.push(format!("{name}: {what} is {c:.2}:1 ({fg:06X} on {bg:06X})"));
        }
    }
}

#[test]
fn text_and_status_colours_meet_wcag_aa() {
    let mut problems = vec![];
    check("leon-dark", &DARK, &mut problems);
    check("leon-light", &LIGHT, &mut problems);
    for i in (1..=6).chain(8..=14) {
        let c = contrast(LIGHT.ansi[i], LIGHT.bg);
        if c < 4.5 {
            problems.push(format!("leon-light: ANSI {i} is {c:.2}:1 on the pane"));
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}
