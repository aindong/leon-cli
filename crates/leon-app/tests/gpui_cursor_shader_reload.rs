//! A running app follows its cursor shader: saving the file loads it again (a failure keeps the
//! last good shader and shows why, a fix clears that); a config reload naming another file loads
//! that one; `""` takes the overlay away. The file is looked at on GPUI's clock, off the main
//! thread.

mod layout_support;

use std::time::Duration;

use gpui::{Entity, TestAppContext, VisualTestContext};
use layout_support::*;
use leon_app::view::Workspace;
use leon_lua::rewrite::write_setting;
use serde_json::json;

const GOOD: &str = "void mainImage(out vec4 fragColor, in vec2 fragCoord) {\n    fragColor = texture(iChannel0, fragCoord / iResolution.xy);\n}\n";
const BAD: &str = "void mainImage(out vec4 c, in vec2 p) {\n    c = oops;\n}\n";

fn error(ws: &Entity<Workspace>, cx: &mut VisualTestContext) -> Option<String> {
    ws.read_with(cx, |ws, _| ws.cursor_shader_error().map(String::from))
}

fn has_overlay(ws: &Entity<Workspace>, cx: &mut VisualTestContext) -> bool {
    ws.read_with(cx, |ws, _| ws.cursor_shader_state().is_some())
}

/// Longer than the app waits between two looks at the file.
fn a_look_later(cx: &mut VisualTestContext) {
    cx.executor().advance_clock(Duration::from_millis(1100));
    cx.run_until_parked();
}

#[gpui::test]
fn saving_the_file_loads_it_again(cx: &mut TestAppContext) {
    let d = private_dir();
    let file = d.path().join("trail.glsl");
    std::fs::write(&file, GOOD).unwrap();
    let s = setup_with_config(&format!("leon.set(\"cursor_shader\", {:?})\n", file.to_str().unwrap()));
    let (ws, cx) = open(cx, &s.app);
    ws.update(cx, |ws, cx| ws.sync_keymap(cx));
    assert!(shows(cx, |cx| has_overlay(&ws, cx)), "loaded");
    assert_eq!(error(&ws, cx), None);

    std::fs::write(&file, BAD).unwrap();
    assert!(
        shows(cx, |cx| {
            a_look_later(cx);
            error(&ws, cx).is_some()
        }),
        "a save that fails shows why"
    );
    assert!(error(&ws, cx).unwrap().starts_with("trail.glsl:2: "), "{:?}", error(&ws, cx));
    assert!(has_overlay(&ws, cx), "the last good shader keeps drawing");

    std::fs::write(&file, GOOD.replace("fragColor =", "fragColor  =")).unwrap();
    assert!(
        shows(cx, |cx| {
            a_look_later(cx);
            error(&ws, cx).is_none()
        }),
        "fixing it clears the failure"
    );
    s.app.stop();
}

#[gpui::test]
fn the_setting_names_another_file_or_none(cx: &mut TestAppContext) {
    let d = private_dir();
    let (one, two) = (d.path().join("one.glsl"), d.path().join("two.glsl"));
    std::fs::write(&one, GOOD).unwrap();
    std::fs::write(&two, BAD).unwrap();
    let s = setup_with_config(&format!("leon.set(\"cursor_shader\", {:?})\n", one.to_str().unwrap()));
    let (ws, cx) = open(cx, &s.app);
    ws.update(cx, |ws, cx| ws.sync_keymap(cx));
    assert!(shows(cx, |cx| has_overlay(&ws, cx)));
    let init = s.dir.path().join("init.lua");
    let set = |v: &str, ws: &Entity<Workspace>, cx: &mut VisualTestContext| {
        write_setting(&init, "cursor_shader", &json!(v)).unwrap();
        s.app.call("server.config_reload", json!({})).unwrap();
        ws.update(cx, |ws, cx| ws.sync_keymap(cx));
        cx.run_until_parked();
    };
    set(two.to_str().unwrap(), &ws, cx);
    assert!(shows(cx, |cx| error(&ws, cx).is_some_and(|e| e.starts_with("two.glsl:2: "))), "{:?}", error(&ws, cx));
    set("", &ws, cx);
    assert!(shows(cx, |cx| !has_overlay(&ws, cx)), "none: the overlay goes");
    assert_eq!(error(&ws, cx), None, "and so does its notice");
    s.app.stop();
}
