//! The Settings window's cursor shader rows under Appearance: the file init.lua names, None to
//! clear it, a file picked (as Choose…'s panel returns it) written `~/…` under the home folder,
//! the duration's stepper, and both read-only when init.lua sets them by code. Each change leaves
//! init.lua as `leon config set` would.
mod m9_support;
mod settings_support;
use gpui::TestAppContext;
use serde_json::json;
use settings_support::*;

const SRC: &str =
    "-- mine\nleon.set(\"cursor_shader\", \"~/shaders/smear.glsl\")\nleon.set(\"cursor_shader_duration\", 1)\n";

#[gpui::test]
fn none_clears_the_file_and_the_stepper_steps(cx: &mut TestAppContext) {
    let (o, mut cx) = open_settings(cx, SRC);
    click(&mut cx, "section-appearance");
    assert!(drawn(&mut cx, "cursor-shader-file"), "the file's row");
    click(&mut cx, "cursor-shader-none");
    let cleared = saved(&mut cx, &o) && shows(&mut cx, |cx| value(cx, &o, "cursor_shader") == Some(json!("")));
    let after_none = text(&o.config);
    click(&mut cx, "step-up-cursor_shader_duration");
    let stepped =
        saved(&mut cx, &o) && shows(&mut cx, |cx| value(cx, &o, "cursor_shader_duration") == Some(json!(1.1)));
    let after_step = text(&o.config);
    o.s.app.stop();
    assert!(cleared, "None clears it");
    assert_eq!(after_none, config_set_text(SRC, "cursor_shader", &json!("")));
    assert!(stepped, "the duration steps by 0.1");
    assert_eq!(after_step, config_set_text(&after_none, "cursor_shader_duration", &json!(1.1)));
}

#[gpui::test]
fn a_picked_file_is_written_from_home(cx: &mut TestAppContext) {
    let (o, mut cx) = open_settings(cx, SRC);
    let home = std::path::PathBuf::from(std::env::var_os("HOME").expect("HOME"));
    let picked = home.join("shaders/trail.glsl");
    o.sw.update(&mut cx, |sw, _, cx| sw.pick_cursor_shader(&picked, cx)).unwrap();
    let ok = saved(&mut cx, &o)
        && shows(&mut cx, |cx| value(cx, &o, "cursor_shader") == Some(json!("~/shaders/trail.glsl")));
    let elsewhere = std::path::Path::new("/opt/shaders/glow.glsl");
    o.sw.update(&mut cx, |sw, _, cx| sw.pick_cursor_shader(elsewhere, cx)).unwrap();
    let abs = saved(&mut cx, &o)
        && shows(&mut cx, |cx| value(cx, &o, "cursor_shader") == Some(json!("/opt/shaders/glow.glsl")));
    o.s.app.stop();
    assert!(ok, "a file in the home folder is written ~/…");
    assert!(abs, "one elsewhere as it is");
}

#[gpui::test]
fn set_by_code_it_is_read_only(cx: &mut TestAppContext) {
    let src = "local s = \"~/x.glsl\"\nleon.set(\"cursor_shader\", s)\nleon.set(\"cursor_shader_duration\", 0.5 + 0)\n";
    let (o, mut cx) = open_settings(cx, src);
    click(&mut cx, "section-appearance");
    let locked = drawn(&mut cx, "locked-cursor_shader") && drawn(&mut cx, "locked-cursor_shader_duration");
    o.sw.update(&mut cx, |sw, _, cx| sw.choose("cursor_shader", json!(""), cx)).unwrap();
    cx.run_until_parked();
    let got = text(&o.config);
    o.s.app.stop();
    assert!(locked, "both say why they can't be changed here");
    assert_eq!(got, src, "a read-only row writes nothing");
}
