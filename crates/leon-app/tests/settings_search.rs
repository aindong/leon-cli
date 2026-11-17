//! Settings window: the search field filters rows across every section, by a row's label and by
//! its setting's Lua name; shortcuts by title and action; no match says so.
mod m9_support;
mod settings_support;
use gpui::{TestAppContext, VisualTestContext};
use settings_support::*;

fn shown(cx: &mut VisualTestContext, o: &Opened) -> Vec<String> {
    cx.run_until_parked();
    o.sw.read_with(cx, |sw, _| sw.shown()).unwrap()
}

fn search(cx: &mut VisualTestContext, o: &Opened, query: &str) -> Vec<String> {
    if !o.sw.read_with(cx, |sw, _| sw.query().is_empty()).unwrap() {
        cx.simulate_keystrokes("escape"); // clears the last search's query; the window stays
    }
    cx.simulate_input(query);
    assert_eq!(o.sw.read_with(cx, |sw, _| sw.query().to_string()).unwrap(), query, "typed into the search field");
    shown(cx, o)
}

#[gpui::test]
fn search_finds_rows_by_label_and_by_lua_name(cx: &mut TestAppContext) {
    let (o, mut cx) = open_settings(cx, "");
    let cx = &mut cx;
    assert!(shows(cx, |cx| o.sw.read_with(cx, |sw, _| !sw.shortcuts().is_empty()).unwrap()), "the keymap arrived");

    // by label: "Hint patterns" is in Terminal, the window opened on General
    let got = search(cx, &o, "hint");
    assert!(got.contains(&"hint_patterns".to_string()), "{got:?}");
    assert!(!got.contains(&"resume".to_string()), "{got:?}");

    // by Lua name, which no label says
    let got = search(cx, &o, "fade_inactive");
    assert_eq!(got, vec!["fade_inactive_panes".to_string()]);
    let got = search(cx, &o, "font.size");
    assert!(got.contains(&"font".to_string()), "{got:?}");

    // across sections at once: a setting and shortcuts
    let got = search(cx, &o, "copy");
    assert!(got.contains(&"copy_on_select".to_string()), "{got:?}");
    assert!(got.contains(&"shortcut:pane.copy".to_string()), "{got:?}");
    assert!(got.contains(&"shortcut:client.copy_mode".to_string()), "{got:?}");

    // a shortcut by its action's name
    let got = search(cx, &o, "split.right");
    assert_eq!(got, vec!["shortcut:pane.split.right".to_string()]);

    // one row per binding: ⌘3 alone, though ⌘1…⌘9 all run client.tab.goto
    let got = search(cx, &o, "go to tab 3");
    assert_eq!(got, vec!["shortcut:client.tab.goto".to_string()]);

    let got = search(cx, &o, "zzzz");
    assert!(got.is_empty(), "{got:?}");
    assert!(drawn(cx, "no-results"), "no match says so");

    // Escape clears the query and shows the section again
    cx.simulate_keystrokes("escape");
    assert_eq!(o.sw.read_with(cx, |sw, _| sw.query().to_string()).unwrap(), "");
    let got = shown(cx, &o);
    assert!(got.contains(&"resume".to_string()) && got.contains(&"check_for_updates".to_string()), "{got:?}");
    let open = cx.windows().len();
    o.s.app.stop();
    assert_eq!(open, 1, "Escape on a query doesn't close the window");
}

#[gpui::test]
fn a_backspace_edits_the_query(cx: &mut TestAppContext) {
    let (o, mut cx) = open_settings(cx, "");
    cx.simulate_input("hintx");
    cx.simulate_keystrokes("backspace");
    let q = o.sw.read_with(&cx, |sw, _| sw.query().to_string()).unwrap();
    let got = shown(&mut cx, &o);
    o.s.app.stop();
    assert_eq!(q, "hint");
    assert!(got.contains(&"hint_patterns".to_string()), "{got:?}");
}

/// A binding init.lua adds is found too, beside the default for the same action.
#[gpui::test]
fn search_finds_a_config_binding(cx: &mut TestAppContext) {
    let (o, mut cx) = open_settings(cx, "leon.bind(\"cmd+shift+y\", \"pane.zoom\")\n");
    assert!(shows(&mut cx, |cx| o.sw.read_with(cx, |sw, _| !sw.shortcuts().is_empty()).unwrap()));
    let got = search(&mut cx, &o, "zoom pane");
    let keys: Vec<String> =
        o.sw.read_with(&cx, |sw, _| sw.shortcuts())
            .unwrap()
            .into_iter()
            .filter(|r| r.title == "Zoom Pane")
            .map(|r| r.keys)
            .collect();
    o.s.app.stop();
    assert_eq!(got, vec!["shortcut:pane.zoom".to_string(); 2], "the default and init.lua's");
    assert!(keys.contains(&"⌘⇧Y".to_string()), "{keys:?}");
}
