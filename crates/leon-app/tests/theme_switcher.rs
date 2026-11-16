//! Themes criteria 3–5 (.agent/plans/2026-11-17-themes.md): "Change Theme…" in the palette lists
//! every theme, built in and imported; moving through the list previews each on the window; esc
//! goes back to the theme in use before; ↩ keeps it, written to init.lua for the side macOS is on
//! (both sides when the setting names one theme for both).

mod m10b_support;

use gpui::{Entity, TestAppContext, VisualTestContext};
use leon_app::theme::{builtin, BUILTIN, DARK, LIGHT};
use leon_app::view::Workspace;
use m10b_support::*;

/// A theme file `leon theme import` would have stored.
const MINE: &str = "background = #101418\nforeground = #c8d0d8\n";

fn titles(ws: &Entity<Workspace>, cx: &mut VisualTestContext) -> Vec<String> {
    ws.read_with(cx, |ws, _| ws.palette_rows()).into_iter().map(|(t, _)| t).collect()
}

/// init.lua's theme as (dark, light).
fn setting(s: &Setup) -> (String, String) {
    let cfg = leon_lua::load(&s.dir.path().join("init.lua")).expect("init.lua loads");
    (cfg.settings.theme.dark, cfg.settings.theme.light)
}

fn init_lua(s: &Setup) -> String {
    std::fs::read_to_string(s.dir.path().join("init.lua")).unwrap()
}

#[gpui::test]
fn the_palette_offers_change_theme(cx: &mut TestAppContext) {
    let s = setup_with_config("-- mine\n");
    let (ws, cx) = open(cx, &s.app);
    let rows = ws.read_with(cx, |ws, _| ws.palette_rows_for("theme"));
    s.app.stop();
    assert!(rows.contains(&("Change Theme…".to_string(), Some("⌘⇧T".to_string()))), "{rows:?}");
}

#[gpui::test]
fn the_list_has_every_theme_and_esc_goes_back(cx: &mut TestAppContext) {
    let s = setup_with_config("-- mine\n");
    std::fs::create_dir_all(s.dir.path().join("themes")).unwrap();
    std::fs::write(s.dir.path().join("themes/mine.theme"), MINE).unwrap();
    let (ws, cx) = open(cx, &s.app);
    ws.update(cx, |ws, _| ws.sync_keymap());
    let before = init_lua(&s);
    assert_eq!(ws.read_with(cx, |ws, _| ws.theme()), DARK);

    cx.simulate_keystrokes("cmd-shift-t");
    assert!(ws.read_with(cx, |ws, _| ws.palette_open()), "⌘⇧T opens the theme list");
    let prompt = ws.read_with(cx, |ws, _| ws.palette_prompt());
    let listed = titles(&ws, cx);
    let mut want: Vec<String> = BUILTIN.iter().map(|(n, _)| n.to_string()).collect();
    want.push("mine".into());
    let selected = ws.read_with(cx, |ws, _| ws.palette_selected_row());

    cx.simulate_keystrokes("down");
    let previewed = ws.read_with(cx, |ws, _| ws.theme());
    let picked = titles(&ws, cx)[1].clone();
    cx.simulate_keystrokes("down");
    let next = ws.read_with(cx, |ws, _| ws.theme());
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    let back = ws.read_with(cx, |ws, _| (ws.theme(), ws.palette_open()));
    // ⌘K inside the list turns the palette into the switcher: the preview goes with the list
    cx.simulate_keystrokes("cmd-shift-t down cmd-k");
    let switched = ws.read_with(cx, |ws, _| (ws.theme(), ws.switcher_open()));
    cx.simulate_keystrokes("escape");
    s.app.stop();

    assert_eq!(listed, want, "every built-in theme, then the stored one");
    assert_eq!(prompt, "Theme for both appearances…", "init.lua names one theme for both");
    assert_eq!(selected, Some(0), "the theme in use (leon-dark) is selected");
    assert_eq!(picked, "leon-light");
    assert_eq!(previewed, LIGHT, "moving down previews leon-light");
    assert_eq!(Some(next), builtin(&want[2]), "and the next one");
    assert_eq!(back, (DARK, false), "esc closes the list and goes back");
    assert_eq!(switched, (DARK, true), "⌘K in the list goes back too");
    assert_eq!(init_lua(&s), before, "a preview writes nothing");
}

#[gpui::test]
fn enter_keeps_one_theme_for_both_sides(cx: &mut TestAppContext) {
    let s = setup_with_config("-- mine\n");
    let (ws, cx) = open(cx, &s.app);
    ws.update(cx, |ws, _| ws.sync_keymap());
    let sand = builtin("leon-sand").unwrap();
    act(&ws, cx, "client.theme.change");
    cx.simulate_input("sand");
    cx.simulate_keystrokes("enter");
    let at_once = ws.read_with(cx, |ws, _| (ws.theme(), ws.palette_open()));
    // never back to the old theme while init.lua is written and the server reloads
    let mut wavered = vec![];
    let written = shows(cx, |cx| {
        let t = ws.read_with(cx, |ws, _| ws.theme());
        if t != sand {
            wavered.push(t.bg);
        }
        setting(&s) == ("leon-sand".into(), "leon-sand".into())
    });
    let got = setting(&s);
    ws.update(cx, |ws, _| ws.sync_keymap());
    let after_reload = ws.read_with(cx, |ws, _| ws.theme());
    s.app.stop();
    assert_eq!(at_once, (sand, false), "↩ closes the list and keeps leon-sand at once");
    assert!(written, "init.lua names leon-sand for both appearances: {got:?}");
    assert!(wavered.is_empty(), "the theme went back meanwhile: {wavered:x?}");
    assert_eq!(after_reload, sand, "the window keeps drawing with it after the server reloads");
}

#[gpui::test]
fn enter_changes_only_the_side_macos_is_on(cx: &mut TestAppContext) {
    let s = setup_with_config("leon.set(\"theme\", { dark = \"leon-dark\", light = \"leon-light\" })\n");
    let (ws, cx) = open(cx, &s.app);
    ws.update(cx, |ws, _| ws.sync_keymap());
    assert_eq!(ws.read_with(cx, |ws, _| ws.theme()), LIGHT, "the test window is light");
    act(&ws, cx, "client.theme.change");
    let selected = ws.read_with(cx, |ws, _| ws.palette_selected_row());
    let light_prompt = ws.read_with(cx, |ws, _| ws.palette_prompt());
    cx.simulate_input("linen");
    cx.simulate_keystrokes("enter");
    let light = shows(cx, |_| setting(&s) == ("leon-dark".into(), "leon-linen".into()));
    let after_light = setting(&s);

    ws.update_in(cx, |ws, _, cx| ws.set_appearance(true, cx));
    act(&ws, cx, "client.theme.change");
    let dark_prompt = ws.read_with(cx, |ws, _| ws.palette_prompt());
    cx.simulate_input("tide");
    cx.simulate_keystrokes("enter");
    let dark = shows(cx, |_| setting(&s) == ("leon-tide".into(), "leon-linen".into()));
    let after_dark = setting(&s);
    s.app.stop();
    assert_eq!(selected, Some(1), "leon-light, the light side's theme, is selected");
    assert_eq!(light_prompt, "Theme for when macOS is light…");
    assert_eq!(dark_prompt, "Theme for when macOS is dark…");
    assert!(light, "only the light side changed: {after_light:?}");
    assert!(dark, "then only the dark side: {after_dark:?}");
}

#[gpui::test]
fn a_refused_write_goes_back_and_says_why(cx: &mut TestAppContext) {
    // set by code: `leon config set` can't rewrite the call, so the pick can't be written
    let s = setup_with_config("leon.set(\"theme\", os.getenv(\"LEON_NO_SUCH_VAR\") or \"leon-dark\")\n");
    let (ws, cx) = open(cx, &s.app);
    ws.update(cx, |ws, _| ws.sync_keymap());
    let before = init_lua(&s);
    act(&ws, cx, "client.theme.change");
    cx.simulate_input("sand");
    cx.simulate_keystrokes("enter");
    let reopened = shows(cx, |cx| ws.read_with(cx, |ws, _| ws.palette_open()));
    let (note, theme) = ws.read_with(cx, |ws, _| (ws.switcher_note(), ws.theme()));
    s.app.stop();
    assert!(reopened, "the list opens again");
    assert!(note.as_deref().is_some_and(|n| n.contains("theme")), "with the reason: {note:?}");
    assert_eq!(theme, DARK, "and the theme in use before");
    assert_eq!(init_lua(&s), before, "init.lua is as it was");
}

#[gpui::test]
fn the_lists_own_key_closes_it_and_the_palettes_key_goes_to_the_actions(cx: &mut TestAppContext) {
    let s = setup_with_config("-- mine\n");
    let (ws, cx) = open(cx, &s.app);
    ws.update(cx, |ws, _| ws.sync_keymap());
    cx.simulate_keystrokes("cmd-shift-t down cmd-shift-t");
    let closed = ws.read_with(cx, |ws, _| (ws.palette_open(), ws.theme()));
    cx.simulate_keystrokes("cmd-shift-t down cmd-shift-p");
    let actions = ws.read_with(cx, |ws, _| (ws.palette_prompt(), ws.theme()));
    cx.simulate_keystrokes("escape");
    s.app.stop();
    assert_eq!(closed, (false, DARK), "⌘⇧T in the list closes it and goes back");
    assert_eq!(actions, ("Type an action…".to_string(), DARK), "⌘⇧P turns it into the action list");
}
