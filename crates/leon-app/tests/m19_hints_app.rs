//! M19 criterion 11 (.agent/plans/2026-11-15-m19-tmux-migrants.md): hints in the app: ⌘⇧H labels
//! what is on screen, a label copies, the label in upper case pastes into the pane, Esc leaves,
//! and the config's `hint_patterns` add matches.
mod m11_support;

use gpui::TestAppContext;
use m11_support::*;

const SCRIPT: &str = "echo 'see https://example.com/x and src/main.rs at 3f2a9c1 or TICKET-42'; exec cat";

fn hints(
    ws: &gpui::Entity<leon_app::view::Workspace>,
    cx: &mut gpui::VisualTestContext,
    id: &str,
) -> Vec<(String, String)> {
    cx.run_until_parked();
    ws.read_with(cx, |ws, _| ws.hint_labels(id))
}

fn label_of(found: &[(String, String)], text: &str) -> String {
    found.iter().find(|(_, t)| t == text).unwrap_or_else(|| panic!("no hint for {text}: {found:?}")).0.clone()
}

/// Type `label` as keystrokes, each letter in upper case when `upper`.
fn type_label(cx: &mut gpui::VisualTestContext, label: &str, upper: bool) {
    let keys: Vec<String> = label.chars().map(|c| if upper { format!("shift-{c}") } else { c.to_string() }).collect();
    cx.simulate_keystrokes(&keys.join(" "));
    cx.run_until_parked();
}

#[gpui::test]
fn labels_copy_and_insert(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let id = tab_running(&s, &ws, cx, SCRIPT, "TICKET-42");
    cx.simulate_keystrokes("cmd-shift-h");
    let found = hints(&ws, cx, &id);
    let mut texts: Vec<&str> = found.iter().map(|(_, t)| t.as_str()).collect();
    texts.sort();
    assert_eq!(texts, ["3f2a9c1", "https://example.com/x", "src/main.rs"], "no config: no TICKET hint");
    let drawn = ws.read_with(cx, |ws, _| ws.drawn_text(&id)).unwrap();
    let row = drawn.lines().find(|l| l.contains("example.com")).expect("the line is drawn");
    let url = label_of(&found, "https://example.com/x");
    assert!(row[4..].starts_with(&url), "the URL's label is drawn over its start: {row:?}");

    type_label(cx, &url, false);
    assert_eq!(clipboard(cx).as_deref(), Some("https://example.com/x"));
    assert!(hints(&ws, cx, &id).is_empty(), "a label leaves hints");

    cx.simulate_keystrokes("cmd-shift-h");
    let path = label_of(&hints(&ws, cx, &id), "src/main.rs");
    type_label(cx, &path, true);
    assert!(hints(&ws, cx, &id).is_empty());
    let pasted = shows(cx, |cx| {
        let screen = ws.read_with(cx, |ws, _| ws.screen_text(&id)).unwrap_or_default();
        screen.lines().any(|l| l.trim_end() == "src/main.rs")
    });
    assert!(pasted, "the upper-case label typed the path into the pane (cat echoes it)");

    cx.simulate_keystrokes("cmd-shift-h");
    assert!(!hints(&ws, cx, &id).is_empty());
    cx.simulate_keystrokes("escape");
    assert!(hints(&ws, cx, &id).is_empty(), "Esc leaves");
    s.app.stop();
}

#[gpui::test]
fn the_config_adds_patterns(cx: &mut TestAppContext) {
    let s = setup_with_config(r#"leon.set("hint_patterns", { "TICKET-\\d+" })"#);
    let (ws, cx) = open(cx, &s.app);
    ws.update(cx, |ws, _| ws.sync_keymap());
    let id = tab_running(&s, &ws, cx, SCRIPT, "TICKET-42");
    cx.simulate_keystrokes("cmd-shift-h");
    let found = hints(&ws, cx, &id);
    label_of(&found, "TICKET-42");
    assert_eq!(found.len(), 4);
    s.app.stop();
}
