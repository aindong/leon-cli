//! Cursor shaders in the window (docs/architecture/cursor-shaders.md): only the focused pane of
//! the active window hands the overlay its cursor (a tile under a shown float doesn't); the
//! overlay is off while the window is inactive, Reduce Motion is on, or the overview or palette
//! is open; a shader that fails shows in the notice, which Dismiss hides. GPUI's test window has
//! no AppKit view, so the overlay keeps the cursor state only, which is what is checked here.

mod layout_support;

use gpui::{Entity, TestAppContext, VisualTestContext};
use layout_support::*;
use leon_app::cursor_shader::Cursor;
use leon_app::view::Workspace;
use serde_json::json;

const GOOD: &str = "void mainImage(out vec4 fragColor, in vec2 fragCoord) {\n    fragColor = texture(iChannel0, fragCoord / iResolution.xy);\n}\n";

/// A server whose init.lua names `shader` (written to a file of its own) as the cursor shader.
fn with_shader(shader: &str) -> (Setup, std::path::PathBuf) {
    let d = private_dir();
    let file = d.path().join("trail.glsl");
    std::fs::write(&file, shader).unwrap();
    let s = setup_with_config(&format!("leon.set(\"cursor_shader\", {:?})\n", file.to_str().unwrap()));
    std::mem::forget(d); // the file outlives the test's server
    (s, file)
}

fn state(ws: &Entity<Workspace>, cx: &mut VisualTestContext) -> Option<Cursor> {
    ws.read_with(cx, |ws, _| ws.cursor_shader_state())
}

fn loaded(ws: &Entity<Workspace>, cx: &mut VisualTestContext) {
    ws.update(cx, |ws, cx| ws.sync_keymap(cx));
    assert!(shows(cx, |cx| state(ws, cx).is_some()), "the shader loaded");
}

fn feeds(ws: &Entity<Workspace>, cx: &mut VisualTestContext, id: &str) -> bool {
    ws.read_with(cx, |ws, _| ws.grid_drew(id).is_some_and(|g| g.feeds))
}

fn come_back(cx: &mut VisualTestContext) {
    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
}

#[gpui::test]
fn only_the_focused_pane_of_the_active_window_feeds(cx: &mut TestAppContext) {
    let (s, _) = with_shader(GOOD);
    let (ws, cx) = open(cx, &s.app);
    ws.update(cx, |ws, _| ws.set_reduce_motion(false));
    come_back(cx);
    loaded(&ws, cx);
    let sid = sid(&ws, cx);
    let a = ws.read_with(cx, |ws, _| ws.target_block()).unwrap();
    let b = split_quiet(&s, &sid, &a, "b-ready");
    to_rest(&ws, cx);
    assert!(shows(cx, |cx| feeds(&ws, cx, &a) && !feeds(&ws, cx, &b)), "the focused pane feeds, the other doesn't");
    assert!(shows(cx, |cx| state(&ws, cx).is_some_and(|c| c.block() == Some(a.as_str()) && c.u.cursor_visible == 1)));
    assert!(state(&ws, cx).unwrap().enabled, "on in the active window");

    s.app.call("session.focus_block", json!({ "session_id": sid, "block_id": b })).unwrap();
    assert!(shows(cx, |cx| feeds(&ws, cx, &b) && !feeds(&ws, cx, &a)), "the focus moved, and the feed with it");
    assert!(shows(cx, |cx| state(&ws, cx).is_some_and(|c| c.block() == Some(b.as_str()))));

    cx.deactivate_window();
    cx.run_until_parked();
    assert!(shows(cx, |cx| !feeds(&ws, cx, &b)), "an inactive window feeds nothing");
    let c = state(&ws, cx).unwrap();
    assert!(!c.enabled && c.u.focus == 0, "and draws nothing");
    come_back(cx);
    assert!(shows(cx, |cx| state(&ws, cx).is_some_and(|c| c.enabled)), "back on with the window");
    s.app.stop();
}

#[gpui::test]
fn reduce_motion_the_overview_and_the_palette_turn_it_off(cx: &mut TestAppContext) {
    let (s, _) = with_shader(GOOD);
    let (ws, cx) = open(cx, &s.app);
    ws.update(cx, |ws, _| ws.set_reduce_motion(false));
    come_back(cx);
    loaded(&ws, cx);
    let on = |cx: &mut VisualTestContext| state(&ws, cx).is_some_and(|c| c.enabled);
    assert!(shows(cx, |cx| on(cx)));
    ws.update(cx, |ws, cx| {
        ws.set_reduce_motion(true);
        cx.notify();
    });
    assert!(shows(cx, |cx| !on(cx)), "Reduce Motion: off");
    ws.update(cx, |ws, cx| {
        ws.set_reduce_motion(false);
        cx.notify();
    });
    assert!(shows(cx, |cx| on(cx)));
    act(&ws, cx, "client.overview.toggle");
    assert!(shows(cx, |cx| !on(cx)), "the overview: off");
    act(&ws, cx, "client.overview.toggle");
    to_rest(&ws, cx); // the overview's close runs to its end
    assert!(shows(cx, |cx| on(cx)));
    act(&ws, cx, "client.palette.toggle");
    assert!(shows(cx, |cx| !on(cx)), "the palette: off");
    s.app.stop();
}

#[gpui::test]
fn a_tile_under_a_float_doesnt_feed_and_the_float_does(cx: &mut TestAppContext) {
    let (s, _) = with_shader(GOOD);
    let (ws, cx) = open(cx, &s.app);
    come_back(cx);
    loaded(&ws, cx);
    let sid = sid(&ws, cx);
    let a = ws.read_with(cx, |ws, _| ws.target_block()).unwrap();
    let b = split_quiet(&s, &sid, &a, "b-ready");
    let bounds = json!({ "x": 0.3, "y": 0.3, "width": 0.4, "height": 0.4 });
    s.app.call("session.float_block", json!({ "session_id": sid, "block_id": b, "bounds": bounds })).unwrap();
    s.app.call("session.focus_block", json!({ "session_id": sid, "block_id": b })).unwrap();
    assert!(shows(cx, |cx| feeds(&ws, cx, &b)), "a focused float feeds");
    s.app.call("session.focus_block", json!({ "session_id": sid, "block_id": a })).unwrap();
    assert!(
        shows(cx, |cx| ws.read_with(cx, |ws, _| ws.grid_drew(&a).is_some_and(|g| g.active && !g.feeds))),
        "the focused tile under a shown float doesn't: the overlay would draw over the float"
    );
    s.app.stop();
}

#[gpui::test]
fn a_shader_that_fails_shows_in_the_notice_until_dismissed(cx: &mut TestAppContext) {
    let (s, _) = with_shader("void mainImage(out vec4 c, in vec2 p) {\n    c = not_a_thing;\n}\n");
    let (ws, cx) = open(cx, &s.app);
    ws.update(cx, |ws, cx| {
        ws.set_reduce_motion(true); // the notice comes and goes without a fade's frames
        ws.sync_keymap(cx);
    });
    let error = |cx: &mut VisualTestContext| ws.read_with(cx, |ws, _| ws.cursor_shader_error().map(String::from));
    assert!(shows(cx, |cx| error(cx).is_some()), "the load failed");
    let e = error(cx).unwrap();
    assert!(e.starts_with("trail.glsl:2: ") && e.contains("not_a_thing"), "{e}");
    assert!(shows(cx, |cx| drawn(cx, "update-notice")), "the notice shows it");
    assert!(state(&ws, cx).is_none(), "no overlay: the app runs without the effect");
    act(&ws, cx, "client.update.dismiss");
    assert!(shows(cx, |cx| !drawn(cx, "update-notice")), "Dismiss hides it");
    s.app.stop();
}
