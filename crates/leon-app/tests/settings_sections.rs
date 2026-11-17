//! Settings window: a sidebar of sections a click or ↑/↓ moves through, ⌘W and Escape close it;
//! Shortcuts lists the keymap and opens init.lua; Agents shows `leon skill status`; the window
//! is as translucent as the main one and drawn in the theme; idle, it draws nothing.
mod m9_support;
mod settings_support;
use std::time::Duration;

use gpui::{TestAppContext, VisualTestContext, WindowBackgroundAppearance as Bg};
use leon_app::glass::Glass;
use leon_app::keymap::Keymap;
use leon_app::settings_window::Section;
use serde_json::json;
use settings_support::*;

fn section(cx: &mut VisualTestContext, o: &Opened) -> Section {
    o.sw.read_with(cx, |sw, _| sw.section()).unwrap()
}

fn shown(cx: &mut VisualTestContext, o: &Opened) -> Vec<String> {
    cx.run_until_parked();
    o.sw.read_with(cx, |sw, _| sw.shown()).unwrap()
}

#[gpui::test]
fn clicking_a_section_shows_it(cx: &mut TestAppContext) {
    let (o, mut cx) = open_settings(cx, "");
    let cx = &mut cx;
    assert_eq!(section(cx, &o), Section::General, "it opens on General");
    let each: [(&'static str, Section, &str); 6] = [
        ("section-appearance", Section::Appearance, "translucency"),
        ("section-terminal", Section::Terminal, "repeat_time"),
        ("section-notifications", Section::Notifications, "notifications.blocked"),
        ("section-shortcuts", Section::Shortcuts, "shortcut:pane.split.right"),
        ("section-agents", Section::Agents, "agents"),
        ("section-general", Section::General, "resume"),
    ];
    for (sel, want, row) in each {
        click(cx, sel);
        assert_eq!(section(cx, &o), want, "{sel}");
        assert!(shows(cx, |cx| shown(cx, &o).contains(&row.to_string())), "{sel} shows {row}: {:?}", shown(cx, &o));
        assert!(!shown(cx, &o).contains(&"theme".to_string()) || want == Section::Appearance, "one section at a time");
    }
    o.s.app.stop();
}

#[gpui::test]
fn keys_move_and_close(cx: &mut TestAppContext) {
    let (o, mut vcx) = open_settings(cx, "");
    vcx.simulate_keystrokes("down");
    assert_eq!(section(&mut vcx, &o), Section::Appearance);
    vcx.simulate_keystrokes("down down");
    assert_eq!(section(&mut vcx, &o), Section::Notifications);
    vcx.simulate_keystrokes("up");
    assert_eq!(section(&mut vcx, &o), Section::Terminal);
    vcx.simulate_keystrokes("up up up");
    assert_eq!(section(&mut vcx, &o), Section::General, "stops at the top");
    vcx.simulate_keystrokes("cmd-w");
    let left = vcx.windows().len();
    o.s.app.stop();
    assert_eq!(left, 0, "⌘W closes it");

    let (o, mut vcx) = open_settings(cx, "");
    vcx.simulate_keystrokes("escape");
    let left = vcx.windows().len();
    o.s.app.stop();
    assert_eq!(left, 0, "Escape with no query closes it");
}

#[gpui::test]
fn shortcuts_lists_the_keymap(cx: &mut TestAppContext) {
    let (o, mut vcx) = open_settings(cx, "leon.bind(\"cmd+shift+y\", \"pane.zoom\")\n");
    let km = o.s.app.call("client.keymap", json!({})).unwrap();
    let want = Keymap::app(&km).bindings().len();
    let rows = |cx: &mut VisualTestContext| o.sw.read_with(cx, |sw, _| sw.shortcuts()).unwrap();
    assert!(shows(&mut vcx, |cx| rows(cx).len() == want), "every binding: {} of {want}", rows(&mut vcx).len());
    let rows = rows(&mut vcx);
    let has = |keys: &str, title: &str, action: &str| {
        rows.iter().any(|r| r.keys == keys && r.title == title && r.action.as_deref() == Some(action))
    };
    assert!(has("⌘⇧Y", "Zoom Pane", "pane.zoom"), "init.lua's binding: {rows:?}");
    assert!(has("⌘D", "Split Right", "pane.split.right"), "a default: {rows:?}");
    click(&mut vcx, "section-shortcuts");
    click(&mut vcx, "edit-init-lua");
    let opened = vcx.opened_url();
    o.s.app.stop();
    assert_eq!(opened, Some(format!("file://{}", o.config.display())), "Edit in init.lua opens the file");
}

#[gpui::test]
fn agents_shows_skill_state(cx: &mut TestAppContext) {
    let (o, mut vcx) = open_settings(cx, "");
    // a stand-in leon printing what `leon --json skill status` prints
    let bin = tempfile::tempdir().unwrap();
    let leon = bin.path().join("leon");
    let status = json!([
        { "agent": "claude", "status": "current", "skill": { "path": "/x/.claude/skills/leon/SKILL.md", "state": "current" },
          "hook": { "path": "/x/.claude/settings.json", "state": "current" } },
        { "agent": "codex", "status": "missing", "skill": { "path": "/x/.agents/skills/leon/SKILL.md", "state": "missing" },
          "hook": { "path": "/x/.codex/hooks.json", "state": "missing" } }
    ]);
    let script = format!("#!/bin/sh\n[ \"$*\" = \"--json skill status\" ] || exit 9\ncat <<'EOF'\n{status}\nEOF\n");
    std::fs::write(&leon, script).unwrap();
    std::os::unix::fs::PermissionsExt::set_mode(&mut std::fs::metadata(&leon).unwrap().permissions(), 0o755);
    std::fs::set_permissions(&leon, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    o.sw.update(&mut vcx, |sw, _, cx| sw.read_agents(leon.clone(), cx)).unwrap();
    let agents = |cx: &mut VisualTestContext| o.sw.read_with(cx, |sw, _| sw.agents()).unwrap();
    assert!(shows(&mut vcx, |cx| agents(cx).is_some()), "the status arrived");
    let got = agents(&mut vcx).unwrap().expect("parsed");
    click(&mut vcx, "section-agents");
    let rows = shown(&mut vcx, &o);
    o.s.app.stop();
    assert_eq!(got.len(), 2);
    assert_eq!((got[0].agent.as_str(), got[0].status.as_str(), got[0].setup.as_deref()), ("claude", "current", None));
    assert_eq!(got[1].setup.as_deref(), Some("leon skill setup --agent codex"), "how to set it up");
    assert!(rows.contains(&"agent:claude".to_string()) && rows.contains(&"agent:codex".to_string()), "{rows:?}");
}

#[gpui::test]
fn the_window_is_as_translucent_as_the_main_one(cx: &mut TestAppContext) {
    let (o, mut vcx) = open_settings(cx, "leon.set(\"translucency\", 0.5)\n");
    o.sw.update(&mut vcx, |sw, window, cx| sw.set_reduce_transparency(Some(false), window, cx)).unwrap();
    let (bg, alpha, theme) =
        o.sw.read_with(&vcx, |sw, _| (sw.backgrounds_set().last().copied(), sw.fill_alpha(), sw.theme())).unwrap();
    assert_eq!(bg, Some(Bg::Blurred));
    assert_eq!(alpha, Glass::new(&theme, 0.5, 0.).chrome_alpha(), "the chrome's alpha, as the main window's");
    assert!(alpha < 1.);
    o.sw.update(&mut vcx, |sw, window, cx| sw.set_reduce_transparency(Some(true), window, cx)).unwrap();
    let (bg, alpha) = o.sw.read_with(&vcx, |sw, _| (sw.backgrounds_set().last().copied(), sw.fill_alpha())).unwrap();
    o.s.app.stop();
    assert_eq!((bg, alpha), (Some(Bg::Opaque), 1.), "Reduce Transparency: opaque");

    let (o, mut vcx) = open_settings(cx, "leon.set(\"translucency\", 0)\n");
    o.sw.update(&mut vcx, |sw, window, cx| sw.set_reduce_transparency(Some(false), window, cx)).unwrap();
    let (bg, alpha) = o.sw.read_with(&vcx, |sw, _| (sw.backgrounds_set().last().copied(), sw.fill_alpha())).unwrap();
    o.s.app.stop();
    assert_eq!((bg, alpha), (Some(Bg::Opaque), 1.), "0 is opaque");
}

#[gpui::test]
fn the_window_is_drawn_in_the_chosen_theme(cx: &mut TestAppContext) {
    let (o, mut vcx) = open_settings(cx, "");
    let tide = leon_core::theme::builtin("leon-tide").unwrap();
    assert_ne!(o.sw.read_with(&vcx, |sw, _| sw.theme()).unwrap(), tide);
    click(&mut vcx, "section-appearance");
    click(&mut vcx, "theme-dark-leon-tide");
    click(&mut vcx, "theme-light-leon-tide");
    let ok = saved(&mut vcx, &o) && shows(&mut vcx, |cx| o.sw.read_with(cx, |sw, _| sw.theme()).unwrap() == tide);
    o.s.app.stop();
    assert!(ok, "the window follows the theme it set");
}

#[gpui::test]
fn idle_it_draws_nothing(cx: &mut TestAppContext) {
    let (o, mut vcx) = open_settings(cx, "");
    let renders = |cx: &mut VisualTestContext| o.sw.read_with(cx, |sw, _| sw.renders()).unwrap();
    for _ in 0..3 {
        vcx.executor().advance_clock(Duration::from_millis(1100));
        vcx.run_until_parked();
    }
    let before = renders(&mut vcx);
    for _ in 0..5 {
        vcx.executor().advance_clock(Duration::from_millis(1100));
        vcx.run_until_parked();
    }
    let after = renders(&mut vcx);
    o.s.app.stop();
    assert_eq!(after, before, "no renders while nothing changes");
}

/// With no config folder yet, Edit in init.lua makes it and an empty init.lua, as `leon config
/// set` would, so the editor has a file to open.
#[gpui::test]
fn edit_in_init_lua_makes_its_folder(cx: &mut TestAppContext) {
    let s = m9_support::setup_with_config("");
    let config = s.dir.path().join("fresh").join("leon").join("init.lua");
    let remote = std::rc::Rc::new(leon_app::remote::Remote::connect(s.app.socket()).expect("attached"));
    let at = config.clone();
    let sw = cx.update(|cx| leon_app::settings_window::SettingsWindow::open(at, remote, cx)).expect("opened");
    let mut vcx = VisualTestContext::from_window(sw.into(), cx);
    vcx.simulate_resize(gpui::size(gpui::px(900.), gpui::px(2400.)));
    assert!(shows(&mut vcx, |cx| sw.read_with(cx, |sw, _| sw.row("theme").is_some()).unwrap()));
    click(&mut vcx, "section-shortcuts");
    click(&mut vcx, "edit-init-lua");
    let err = sw.read_with(&vcx, |sw, _| sw.error()).unwrap();
    let made = config.exists();
    s.app.stop();
    assert!(made, "init.lua not made: {err:?}");
    assert_eq!(vcx.opened_url(), Some(format!("file://{}", config.display())));
}

/// An init.lua that doesn't load leaves its own rows blank and says why, but the sections that
/// don't come from it (Shortcuts, Agents) still work.
#[gpui::test]
fn a_broken_init_lua_leaves_the_other_sections(cx: &mut TestAppContext) {
    let s = m9_support::setup();
    let config = s.dir.path().join("init.lua");
    std::fs::write(&config, "leon.set(\"font.size\", ").unwrap(); // a syntax error
    let remote = std::rc::Rc::new(leon_app::remote::Remote::connect(s.app.socket()).unwrap());
    let sw = cx.update(|cx| leon_app::settings_window::SettingsWindow::open(config, remote, cx)).unwrap();
    let mut vcx = VisualTestContext::from_window(sw.into(), cx);
    vcx.simulate_resize(gpui::size(gpui::px(900.), gpui::px(2400.)));
    assert!(shows(&mut vcx, |cx| sw.read_with(cx, |sw, _| sw.error()).unwrap().is_some()), "it says why");
    click(&mut vcx, "section-shortcuts");
    let ok = shows(&mut vcx, |cx| drawn(cx, "edit-init-lua"));
    click(&mut vcx, "section-general");
    let blank = !drawn(&mut vcx, "choice-resume-ask");
    s.app.stop();
    assert!(ok, "Shortcuts still shows, with Edit in init.lua to fix the file");
    assert!(blank, "no control shows a value init.lua didn't give");
}

/// Long text wraps inside the window; it never widens the page past it, which would leave
/// controls out of reach to the right.
#[gpui::test]
fn the_page_fits_the_window(cx: &mut TestAppContext) {
    let (o, mut vcx) = open_settings(cx, "");
    click(&mut vcx, "section-general");
    let width = 900.; // open_settings' window
    let right = |cx: &mut VisualTestContext, sel: &'static str| cx.debug_bounds(sel).map(|b| f32::from(b.right()));
    let resume = right(&mut vcx, "choice-resume-never");
    click(&mut vcx, "section-server");
    let got = [resume, right(&mut vcx, "tailscale-section")];
    o.s.app.stop();
    for r in got {
        assert!(r.is_some_and(|r| r <= width), "{got:?} past {width}");
    }
}
