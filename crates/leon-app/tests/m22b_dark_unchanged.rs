//! M22b criterion 1: with no config the app draws exactly today's colours and font, and every
//! surface reads the workspace's theme rather than a constant.
mod m9_support;
use gpui::TestAppContext;
use leon_app::theme::DARK;
use m9_support::*;

#[gpui::test]
fn no_config_draws_dark_menlo_13(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    ws.update(cx, |ws, cx| ws.sync_keymap(cx));
    let (theme, family, size) = ws.read_with(cx, |ws, _| (ws.theme(), ws.font_family(), ws.font_size()));
    let t = focused(&s.app, &ws, cx);
    run_until(&t, "printf '\\112\\033[31m\\121\\033[0m\\n'; echo done", "done");
    let id = ws.read_with(cx, |ws, _| ws.target_block()).unwrap();
    let pane = |cx: &mut gpui::VisualTestContext| (fg_of(&ws, cx, &id, "J"), fg_of(&ws, cx, &id, "Q"));
    let seen = shows(cx, |cx| pane(cx) == (Some(DARK.fg), Some(DARK.ansi[1])));
    let got = pane(cx);
    s.app.stop();
    assert_eq!(theme, DARK, "the default theme is today's");
    assert_eq!((family.as_str(), size), ("Menlo", 13.));
    assert!(seen, "the pane's text comes in today's colours: {got:x?}");
}

/// Only the theme module and the code that picks a theme name `DARK` or `LIGHT`: every view
/// draws with the workspace's current theme, and with the font init.lua names.
#[test]
fn views_draw_with_the_current_theme() {
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut found = vec![];
    for e in std::fs::read_dir(&src).unwrap() {
        let p = e.unwrap().path();
        let name = p.file_name().unwrap().to_string_lossy().into_owned();
        if p.is_dir() || name == "theme.rs" || name == "look.rs" {
            continue;
        }
        let text = std::fs::read_to_string(&p).unwrap();
        for (i, line) in text.lines().enumerate() {
            let word = |w: &str| {
                line.match_indices(w).any(|(at, _)| {
                    let before = line[..at].chars().last().is_none_or(|c| !c.is_alphanumeric() && c != '_');
                    let after = line[at + w.len()..].chars().next().is_none_or(|c| !c.is_alphanumeric() && c != '_');
                    before && after
                })
            };
            if word("DARK") || word("LIGHT") || line.contains("FONT_FAMILY") {
                found.push(format!("{name}:{}: {}", i + 1, line.trim()));
            }
        }
    }
    assert!(found.is_empty(), "views that name a theme constant:\n{}", found.join("\n"));
}

/// The colour the app received for the last span reading `text` in pane `block_id`'s screen.
fn fg_of(
    ws: &gpui::Entity<leon_app::view::Workspace>,
    cx: &mut gpui::VisualTestContext,
    block_id: &str,
    text: &str,
) -> Option<u32> {
    let f = ws.read_with(cx, |ws, _| ws.screen(block_id))?;
    f.rows.iter().flatten().rfind(|s| s.text == text).map(|s| s.fg)
}
