//! Settings window: every setting in docs/lua/settings.md has a control. Each control, clicked in
//! the window, starts from init.lua's value and leaves init.lua exactly as `leon config set` would
//! for the new value; a number stops at its range; a bad hint pattern says why inline; a setting
//! init.lua sets by code can't be changed and says so.
mod m9_support;
mod settings_support;
use gpui::TestAppContext;
use leon_app::settings_window::SettingsWindow;
use serde_json::{json, Value};
use settings_support::*;

/// Every setting, set to something other than its default, so a control must read the file.
const SRC: &str = r#"-- mine
leon.set("theme", { dark = "leon-dark", light = "leon-light" })
leon.set("font.family", "Menlo")
leon.set("font.size", 14)
leon.set("copy_on_select", false)
leon.set("repeat_time", 700)
leon.set("hint_patterns", { "abc" })
leon.set("resume", "never")
leon.set("fade_inactive_panes", 0.4)
leon.set("check_for_updates", true)
leon.set("translucency", 0.2)
leon.set("scrollback", 5000)
"#;

/// Open the window on `SRC`, show `section`, click `control`, and check `key` went from `before`
/// to `after` in the window and in init.lua.
fn one(cx: &mut TestAppContext, section: &'static str, control: &'static str, key: &str, before: Value, after: Value) {
    let (o, mut cx) = open_settings(cx, SRC);
    assert_eq!(value(&mut cx, &o, key), Some(before), "{key}: the control shows init.lua's value");
    click(&mut cx, section);
    click(&mut cx, control);
    let ok = saved(&mut cx, &o) && shows(&mut cx, |cx| value(cx, &o, key) == Some(after.clone()));
    let (got, error) = (text(&o.config), o.sw.read_with(&cx, |sw, _| sw.error()).unwrap());
    o.s.app.stop();
    assert!(ok, "{key}: the window never showed {after} ({error:?})");
    assert_eq!(got, config_set_text(SRC, key, &after), "{key}: init.lua as `leon config set` writes it");
}

#[test]
fn every_key_has_a_control() {
    let mut have = SettingsWindow::controls();
    let mut want = leon_lua::settings::KEYS.to_vec();
    have.sort();
    want.sort();
    assert_eq!(have, want, "a control for each setting in docs/lua/settings.md");
}

#[gpui::test]
fn theme(cx: &mut TestAppContext) {
    let before = json!({ "dark": "leon-dark", "light": "leon-light" });
    one(
        cx,
        "section-appearance",
        "theme-dark-leon-tide",
        "theme",
        before,
        json!({ "dark": "leon-tide", "light": "leon-light" }),
    );
}

#[gpui::test]
fn font_family(cx: &mut TestAppContext) {
    let (o, mut cx) = open_settings(cx, SRC);
    assert_eq!(value(&mut cx, &o, "font.family"), Some(json!("Menlo")));
    click(&mut cx, "section-appearance");
    if !drawn(&mut cx, "font-next") {
        // the test platform knows fewer than two of the families: the control is inert then
        o.s.app.stop();
        return;
    }
    click(&mut cx, "font-next");
    assert!(saved(&mut cx, &o));
    let now = value(&mut cx, &o, "font.family").unwrap();
    let got = text(&o.config);
    o.s.app.stop();
    assert_ne!(now, json!("Menlo"), "the next family");
    assert_eq!(got, config_set_text(SRC, "font.family", &now));
}

#[gpui::test]
fn font_size(cx: &mut TestAppContext) {
    one(cx, "section-appearance", "step-up-font.size", "font.size", json!(14.0), json!(15.0));
}

#[gpui::test]
fn notifications_done(cx: &mut TestAppContext) {
    one(cx, "section-notifications", "toggle-notifications.done", "notifications.done", json!(true), json!(false));
}

#[gpui::test]
fn notifications_blocked(cx: &mut TestAppContext) {
    one(
        cx,
        "section-notifications",
        "toggle-notifications.blocked",
        "notifications.blocked",
        json!(true),
        json!(false),
    );
}

#[gpui::test]
fn notifications_error(cx: &mut TestAppContext) {
    one(cx, "section-notifications", "toggle-notifications.error", "notifications.error", json!(true), json!(false));
}

#[gpui::test]
fn copy_on_select(cx: &mut TestAppContext) {
    one(cx, "section-terminal", "toggle-copy_on_select", "copy_on_select", json!(false), json!(true));
}

#[gpui::test]
fn repeat_time(cx: &mut TestAppContext) {
    one(cx, "section-terminal", "step-up-repeat_time", "repeat_time", json!(700), json!(750));
}

#[gpui::test]
fn scrollback(cx: &mut TestAppContext) {
    one(cx, "section-terminal", "step-up-scrollback", "scrollback", json!(5000), json!(6000));
}

#[gpui::test]
fn resume(cx: &mut TestAppContext) {
    one(cx, "section-general", "choice-resume-always", "resume", json!("never"), json!("always"));
}

#[gpui::test]
fn fade_inactive_panes(cx: &mut TestAppContext) {
    one(cx, "section-appearance", "step-down-fade_inactive_panes", "fade_inactive_panes", json!(0.4), json!(0.35));
}

#[gpui::test]
fn check_for_updates(cx: &mut TestAppContext) {
    one(cx, "section-general", "toggle-check_for_updates", "check_for_updates", json!(true), json!(false));
}

#[gpui::test]
fn translucency(cx: &mut TestAppContext) {
    one(cx, "section-appearance", "step-up-translucency", "translucency", json!(0.2), json!(0.25));
}

#[gpui::test]
fn hint_patterns_remove(cx: &mut TestAppContext) {
    one(cx, "section-terminal", "pattern-remove-0", "hint_patterns", json!(["abc"]), json!([]));
}

#[gpui::test]
fn hint_patterns_add(cx: &mut TestAppContext) {
    let (o, mut cx) = open_settings(cx, SRC);
    assert_eq!(value(&mut cx, &o, "hint_patterns"), Some(json!(["abc"])));
    click(&mut cx, "section-terminal");
    click(&mut cx, "pattern-field");
    cx.simulate_input("x+y");
    cx.simulate_keystrokes("enter");
    let after = json!(["abc", "x+y"]);
    let ok = saved(&mut cx, &o) && shows(&mut cx, |cx| value(cx, &o, "hint_patterns") == Some(after.clone()));
    let got = text(&o.config);
    o.s.app.stop();
    assert!(ok, "the window never showed {after}");
    assert_eq!(got, config_set_text(SRC, "hint_patterns", &after));
}

#[gpui::test]
fn a_bad_pattern_shows_why_and_writes_nothing(cx: &mut TestAppContext) {
    let (o, mut cx) = open_settings(cx, SRC);
    click(&mut cx, "section-terminal");
    click(&mut cx, "pattern-field");
    cx.simulate_input("(x");
    cx.simulate_keystrokes("enter");
    assert!(saved(&mut cx, &o));
    let why = o.sw.read_with(&cx, |sw, _| sw.pattern_error()).unwrap();
    let shown = drawn(&mut cx, "pattern-error");
    let got = text(&o.config);
    o.s.app.stop();
    assert!(why.is_some_and(|w| !w.is_empty()), "the regex's error");
    assert!(shown, "the error is drawn under the field");
    assert_eq!(got, SRC, "nothing written");
}

/// At its range's end a stepper's button does nothing.
#[gpui::test]
fn numbers_stop_at_their_range(cx: &mut TestAppContext) {
    let src = "leon.set(\"repeat_time\", 10000)\nleon.set(\"translucency\", 0.8)\nleon.set(\"fade_inactive_panes\", 0)\nleon.set(\"font.size\", 32)\n";
    let (o, mut cx) = open_settings(cx, src);
    click(&mut cx, "section-terminal");
    click(&mut cx, "step-up-repeat_time");
    click(&mut cx, "section-appearance");
    click(&mut cx, "step-up-translucency");
    click(&mut cx, "step-down-fade_inactive_panes");
    click(&mut cx, "step-up-font.size");
    assert!(saved(&mut cx, &o));
    let got = text(&o.config);
    o.s.app.stop();
    assert_eq!(got, src, "nothing past a range is written");
}

#[gpui::test]
fn a_read_only_control_does_nothing(cx: &mut TestAppContext) {
    let src = "local n = 600 leon.set(\"repeat_time\", n)\n";
    let (o, mut cx) = open_settings(cx, src);
    let r = row(&mut cx, &o, "repeat_time").unwrap();
    assert_eq!(r.value, json!(600));
    assert!(r.read_only.is_some_and(|w| w.contains("init.lua:1")));
    click(&mut cx, "section-terminal");
    assert!(drawn(&mut cx, "locked-repeat_time"), "it says why it can't be changed");
    click(&mut cx, "step-up-repeat_time");
    assert!(saved(&mut cx, &o));
    let (got, now) = (text(&o.config), value(&mut cx, &o, "repeat_time"));
    o.s.app.stop();
    assert_eq!(got, src, "a read-only control writes nothing");
    assert_eq!(now, Some(json!(600)));
}

/// One control of each kind (switch, choice, stepper, the hint list's × and field, the theme
/// list) on a setting init.lua sets by code: clicking it writes nothing, and each says why.
#[gpui::test]
fn every_kind_of_control_is_read_only_when_set_by_code(cx: &mut TestAppContext) {
    let src = r#"local on, how, pats, dark = true, "ask", { "abc" }, "leon-dark"
leon.set("copy_on_select", on)
leon.set("resume", how)
leon.set("hint_patterns", pats)
leon.set("theme", { dark = dark, light = dark })
leon.set("translucency", 0.1 + 0.1)
"#;
    let (o, mut cx) = open_settings(cx, src);
    click(&mut cx, "section-terminal");
    assert!(drawn(&mut cx, "locked-copy_on_select") && drawn(&mut cx, "locked-hint_patterns"));
    click(&mut cx, "toggle-copy_on_select");
    click(&mut cx, "pattern-remove-0");
    assert!(!drawn(&mut cx, "pattern-field"), "no field to add one with");
    click(&mut cx, "section-general");
    assert!(drawn(&mut cx, "locked-resume"));
    click(&mut cx, "choice-resume-always");
    click(&mut cx, "section-appearance");
    assert!(drawn(&mut cx, "locked-theme") && drawn(&mut cx, "locked-translucency"));
    click(&mut cx, "theme-dark-leon-tide");
    click(&mut cx, "step-up-translucency");
    assert!(saved(&mut cx, &o));
    let got = text(&o.config);
    o.s.app.stop();
    assert_eq!(got, src, "nothing written");
}

/// From a hand-written value between steps, a stepper goes to the next step that way, not two.
#[gpui::test]
fn a_step_from_between_steps_goes_to_the_next_one(cx: &mut TestAppContext) {
    let src = "leon.set(\"font.size\", 13.5)\nleon.set(\"repeat_time\", 333)\nleon.set(\"translucency\", 0.33)\n";
    let (o, mut cx) = open_settings(cx, src);
    click(&mut cx, "section-appearance");
    click(&mut cx, "step-up-font.size");
    click(&mut cx, "step-down-translucency");
    click(&mut cx, "section-terminal");
    click(&mut cx, "step-up-repeat_time");
    let want = [("font.size", json!(14.0)), ("translucency", json!(0.3)), ("repeat_time", json!(350))];
    let ok = saved(&mut cx, &o) && shows(&mut cx, |cx| want.iter().all(|(k, v)| value(cx, &o, k).as_ref() == Some(v)));
    let got: Vec<_> = want.iter().map(|(k, _)| value(&mut cx, &o, k)).collect();
    o.s.app.stop();
    assert!(ok, "{got:?}");
}

/// Click a pattern, change it, Enter: that pattern changes in init.lua.
#[gpui::test]
fn hint_patterns_edit(cx: &mut TestAppContext) {
    let (o, mut cx) = open_settings(cx, SRC);
    click(&mut cx, "section-terminal");
    click(&mut cx, "pattern-0");
    cx.simulate_input("d");
    cx.simulate_keystrokes("enter");
    let after = json!(["abcd"]);
    let ok = saved(&mut cx, &o) && shows(&mut cx, |cx| value(cx, &o, "hint_patterns") == Some(after.clone()));
    let got = text(&o.config);
    o.s.app.stop();
    assert!(ok, "the window never showed {after}");
    assert_eq!(got, config_set_text(SRC, "hint_patterns", &after));
}

/// ↑ and ↓ in the hint pattern field stay there: the section doesn't change under it.
#[gpui::test]
fn arrows_in_the_pattern_field_keep_the_section(cx: &mut TestAppContext) {
    let (o, mut cx) = open_settings(cx, "");
    click(&mut cx, "section-terminal");
    click(&mut cx, "pattern-field");
    cx.simulate_keystrokes("down up down");
    let s = o.sw.read_with(&cx, |sw, _| sw.section()).unwrap();
    cx.simulate_keystrokes("cmd-w");
    let left = cx.windows().len();
    o.s.app.stop();
    assert_eq!(s, leon_app::settings_window::Section::Terminal);
    assert_eq!(left, 0, "⌘W still closes from the field");
}
