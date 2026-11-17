//! M22b criterion 3: a running app follows init.lua's settings when they change, without a restart.
mod m9_support;
use gpui::TestAppContext;
use leon_app::theme::{DARK, LIGHT};
use m9_support::*;
use serde_json::json;

#[gpui::test]
fn settings_apply_live(cx: &mut TestAppContext) {
    let s = setup_with_config("-- mine\nleon.set(\"font.size\", 13)\n");
    let (ws, cx) = open(cx, &s.app);
    ws.update(cx, |ws, cx| ws.sync_keymap(cx));
    assert_eq!(ws.read_with(cx, |ws, _| ws.theme()), DARK);
    let id = print_red_y(&s, &ws, cx);

    config_set(&s, "theme", json!("leon-light"));
    config_set(&s, "font.size", json!(15));
    config_set(&s, "font.family", json!("Monaco"));
    config_set(&s, "copy_on_select", json!(true));
    config_set(&s, "notifications.done", json!(false));
    ws.update(cx, |ws, cx| ws.sync_keymap(cx));
    let got = ws.read_with(cx, |ws, _| {
        (ws.theme(), ws.font_size(), ws.font_family(), ws.copy_on_select(), ws.notify_settings().done)
    });
    let screen = shows(cx, |cx| pane(&ws, cx, &id) == (Some(LIGHT.fg), Some(LIGHT.ansi[1])));
    let seen = pane(&ws, cx, &id);
    let scrollback = ws.update(cx, |ws, _| ws.lines(&id, 0, 200)).expect("lines");
    s.app.stop();
    assert!(screen, "the pane's screen comes in the light palette: {seen:x?}");
    let y = scrollback.rows.iter().flat_map(|r| &r.spans).find(|s| s.text == "Q").map(|s| s.fg);
    assert_eq!(y, Some(LIGHT.ansi[1]), "scrollback lines come in the light palette");
    assert_eq!(got.0, LIGHT, "theme");
    assert_eq!(got.1, 15., "font size");
    assert_eq!(got.2, "Monaco", "font family");
    assert!(got.3, "copy_on_select");
    assert!(!got.4, "notifications.done");
}

#[gpui::test]
fn a_pair_switches_with_the_appearance(cx: &mut TestAppContext) {
    let s = setup_with_config("leon.set(\"theme\", { dark = \"leon-dark\", light = \"leon-light\" })\n");
    let (ws, cx) = open(cx, &s.app);
    ws.update(cx, |ws, cx| ws.sync_keymap(cx));
    let id = print_red_y(&s, &ws, cx);
    let light = ws.read_with(cx, |ws, _| ws.theme());
    let light_panes = shows(cx, |cx| pane(&ws, cx, &id) == (Some(LIGHT.fg), Some(LIGHT.ansi[1])));
    ws.update_in(cx, |ws, _, cx| ws.set_appearance(true, cx));
    let dark = ws.read_with(cx, |ws, _| ws.theme());
    let panes = shows(cx, |cx| pane(&ws, cx, &id) == (Some(DARK.fg), Some(DARK.ansi[1])));
    ws.update_in(cx, |ws, _, cx| ws.set_appearance(false, cx));
    let back = ws.read_with(cx, |ws, _| ws.theme());
    s.app.stop();
    assert_eq!(light, LIGHT, "the test window's appearance is light");
    assert_eq!(dark, DARK, "macOS went dark");
    assert_eq!(back, LIGHT, "and back");
    assert!(light_panes, "the panes start light");
    assert!(panes, "the panes went dark with it");
}

#[gpui::test]
fn an_imported_theme_draws(cx: &mut TestAppContext) {
    let s = setup_with_config("");
    std::fs::create_dir_all(s.dir.path().join("themes")).unwrap();
    std::fs::write(s.dir.path().join("themes/harbor.theme"), "background = #102030\nforeground = #e0e0e0\n").unwrap();
    let (ws, cx) = open(cx, &s.app);
    let id = print_red_y(&s, &ws, cx);
    config_set(&s, "theme", json!("harbor"));
    ws.update(cx, |ws, cx| ws.sync_keymap(cx));
    let t = ws.read_with(cx, |ws, _| ws.theme());
    let panes = shows(cx, |cx| pane(&ws, cx, &id).0 == Some(0xe0e0e0));
    s.app.stop();
    assert_eq!((t.bg, t.fg), (0x102030, 0xe0e0e0));
    assert!(panes, "the pane's text is the theme's foreground");
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

/// Print `J` then a red `Q` in the focused pane; returns its block id.
fn print_red_y(s: &Setup, ws: &gpui::Entity<leon_app::view::Workspace>, cx: &mut gpui::VisualTestContext) -> String {
    let t = focused(&s.app, ws, cx);
    run_until(&t, "printf '\\112\\033[31m\\121\\033[0m\\n'; echo done", "done");
    ws.read_with(cx, |ws, _| ws.target_block()).unwrap()
}

/// The colours the app received for `J` and `Q`.
fn pane(
    ws: &gpui::Entity<leon_app::view::Workspace>,
    cx: &mut gpui::VisualTestContext,
    id: &str,
) -> (Option<u32>, Option<u32>) {
    (fg_of(ws, cx, id, "J"), fg_of(ws, cx, id, "Q"))
}
