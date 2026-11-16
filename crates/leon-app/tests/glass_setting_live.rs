//! A running app follows `translucency`: a blurred window with translucent chrome when it is
//! above 0, opaque at 0 without a restart, and opaque with Reduce Transparency on (read again
//! when the window becomes active) or in full screen. GPUI's test window keeps no background, so
//! the app's own log of what it asked GPUI for (`backgrounds_set`) is what is checked.

mod m10b_support;

use gpui::{Entity, TestAppContext, VisualTestContext, WindowBackgroundAppearance as Bg};
use leon_app::view::Workspace;
use leon_lua::rewrite::write_setting;
use m10b_support::*;
use serde_json::json;

/// Longer than the app waits between two looks at the config.
const POLL: std::time::Duration = std::time::Duration::from_millis(1100);

fn config_set(s: &Setup, n: f64) {
    write_setting(&s.dir.path().join("init.lua"), "translucency", &json!(n)).unwrap();
    s.app.call("server.config_reload", json!({})).unwrap();
}

/// What the app last asked GPUI for, and the chrome's alpha.
fn now(ws: &Entity<Workspace>, cx: &mut VisualTestContext) -> (Option<Bg>, f32) {
    ws.read_with(cx, |ws, _| (ws.backgrounds_set().last().copied(), ws.glass().chrome_alpha()))
}

/// [`now`] once the app has asked GPUI for `want` (else what it last asked for).
fn rests_at(ws: &Entity<Workspace>, cx: &mut VisualTestContext, want: Bg) -> (Option<Bg>, f32) {
    shows(cx, |cx| {
        cx.executor().advance_clock(POLL);
        cx.run_until_parked();
        now(ws, cx).0 == Some(want)
    });
    now(ws, cx)
}

/// Leave the window and come back to it, as after changing System Settings.
fn come_back(cx: &mut VisualTestContext) {
    cx.deactivate_window();
    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
}

#[gpui::test]
fn the_window_follows_the_setting(cx: &mut TestAppContext) {
    let s = setup_with_config("leon.set(\"translucency\", 0.3)\n");
    let (ws, cx) = open(cx, &s.app);
    ws.update(cx, |ws, _| ws.set_reduce_transparency(Some(false)));
    come_back(cx); // whatever this Mac's own setting is
    let glass = rests_at(&ws, cx, Bg::Blurred);
    config_set(&s, 0.);
    let opaque = rests_at(&ws, cx, Bg::Opaque);
    config_set(&s, 0.3);
    let again = rests_at(&ws, cx, Bg::Blurred);
    s.app.stop();
    assert_eq!(glass.0, Some(Bg::Blurred), "0.3 blurs what is behind the window");
    assert!(glass.1 < 1., "and the chrome lets it through: {glass:?}");
    assert_eq!(opaque, (Some(Bg::Opaque), 1.), "0 is today's opaque window, without a restart");
    assert_eq!(again.0, Some(Bg::Blurred));
    let set = ws.read_with(cx, |ws, _| ws.backgrounds_set().to_vec());
    assert!(set.ends_with(&[Bg::Blurred, Bg::Opaque, Bg::Blurred]), "GPUI is asked only when it changes: {set:?}");
}

#[gpui::test]
fn reduce_transparency_and_full_screen_are_opaque(cx: &mut TestAppContext) {
    let s = setup_with_config("leon.set(\"translucency\", 0.3)\n");
    let (ws, cx) = open(cx, &s.app);
    ws.update(cx, |ws, _| ws.set_reduce_transparency(Some(false)));
    come_back(cx);
    assert_eq!(rests_at(&ws, cx, Bg::Blurred).0, Some(Bg::Blurred));

    // turned on in System Settings, then back to leon: read again when the window is active
    ws.update(cx, |ws, _| ws.set_reduce_transparency(Some(true)));
    come_back(cx);
    let reduced = now(&ws, cx);
    ws.update(cx, |ws, _| ws.set_reduce_transparency(Some(false)));
    come_back(cx);
    let back = now(&ws, cx).0;

    // entering full screen resizes the window: the bounds observer looks again
    cx.update(|window, _| window.toggle_fullscreen());
    cx.simulate_resize(gpui::size(gpui::px(1500.), gpui::px(950.)));
    cx.run_until_parked();
    let full = now(&ws, cx);
    cx.update(|window, _| window.toggle_fullscreen());
    cx.simulate_resize(gpui::size(gpui::px(1400.), gpui::px(900.)));
    cx.run_until_parked();
    let windowed = now(&ws, cx).0;
    s.app.stop();
    assert_eq!(reduced, (Some(Bg::Opaque), 1.), "Reduce Transparency is opaque");
    assert_eq!(back, Some(Bg::Blurred), "and translucent again when it is off");
    assert_eq!(full, (Some(Bg::Opaque), 1.), "full screen is opaque: nothing is behind it to see");
    assert_eq!(windowed, Some(Bg::Blurred), "and translucent again after it");
}
