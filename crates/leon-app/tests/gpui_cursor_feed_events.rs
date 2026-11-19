//! GPUI phase 5 (docs/architecture/cursor-shaders.md): the cursor shader overlay hears of the
//! focused pane's cursor from the workspace's model events (a pushed screen, a focus change),
//! placed where the grid last painted, not from the grid's paint. GPUI's test window has no
//! AppKit view, so the overlay keeps the cursor state only, which is what is checked.
mod layout_support;

use gpui::{Entity, TestAppContext, VisualTestContext};
use layout_support::*;
use leon_app::cursor_shader::Cursor;
use leon_app::view::Workspace;

const GOOD: &str = "void mainImage(out vec4 fragColor, in vec2 fragCoord) {\n    fragColor = texture(iChannel0, fragCoord / iResolution.xy);\n}\n";

fn state(ws: &Entity<Workspace>, cx: &mut VisualTestContext) -> Option<Cursor> {
    ws.read_with(cx, |ws, _| ws.cursor_shader_state())
}

#[gpui::test]
fn the_overlay_follows_a_pushed_cursor(cx: &mut TestAppContext) {
    let d = private_dir();
    let file = d.path().join("trail.glsl");
    std::fs::write(&file, GOOD).unwrap();
    let s = setup_with_config(&format!("leon.set(\"cursor_shader\", {:?})\n", file.to_str().unwrap()));
    let (ws, cx) = open(cx, &s.app);
    ws.update(cx, |ws, _| ws.set_reduce_motion(false));
    cx.update(|window, _| window.activate_window());
    ws.update(cx, |ws, cx| ws.sync_keymap(cx));
    assert!(shows(cx, |cx| state(&ws, cx).is_some()), "the shader loaded");
    let t = focused(&s.app, &ws, cx);
    run_until(&t, "echo ready", "ready");
    settle(&t);
    let shown = |cx: &mut VisualTestContext| state(&ws, cx).filter(|c| c.u.cursor_visible == 1);
    assert!(shows(cx, |cx| shown(cx).is_some()), "the overlay has the focused pane's cursor");
    let x0 = shown(cx).unwrap().u.current_cursor[0];
    t.write(b"abc").unwrap(); // typed at the prompt, from outside the window: only a pushed screen moves it
    let moved = shows(cx, |cx| shown(cx).is_some_and(|c| c.u.current_cursor[0] > x0));
    assert!(moved, "the overlay's cursor moved with the pushed screen");

    // another tab, then back to the first: its pane is the one the overlay follows again
    let a = ws.read_with(cx, |ws, _| ws.target_block()).unwrap();
    let sid = ws.read_with(cx, |ws, _| ws.session_id()).unwrap();
    let first = window(&s.app, &ws, cx).0.id;
    s.app.call("session.new_window", serde_json::json!({ "session_id": sid })).unwrap();
    let on = |cx: &mut VisualTestContext, id: &str| shown(cx).is_some_and(|c| c.block() == Some(id));
    assert!(shows(cx, |cx| ws.read_with(cx, |ws, _| ws.target_block()).is_some_and(|b| b != a)), "tab 2 shows");
    let b = ws.read_with(cx, |ws, _| ws.target_block()).unwrap();
    assert!(shows(cx, |cx| on(cx, &b)), "the overlay follows tab 2's pane");
    s.app.call("session.focus_window", serde_json::json!({ "session_id": sid, "window_id": first })).unwrap();
    let back = shows(cx, |cx| on(cx, &a));
    s.app.stop();
    assert!(back, "back on tab 1, the overlay has its pane's cursor");
}

/// The feed is the workspace's (view_cursor_shader.rs), never a grid's paint.
#[test]
fn only_the_workspace_feeds_the_overlay() {
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut feeders = Vec::new();
    for e in std::fs::read_dir(&src).unwrap().flatten() {
        let text = std::fs::read_to_string(e.path()).unwrap_or_default();
        if text.contains("overlay.feed(") || text.contains(".feed(window") {
            feeders.push(e.file_name().to_string_lossy().into_owned());
        }
    }
    assert_eq!(feeders, ["view_cursor_shader.rs"], "who feeds the overlay");
}
