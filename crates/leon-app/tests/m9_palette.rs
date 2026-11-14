//! M9 criterion 3: the command palette's fuzzy filter, and the palette in the app.

mod m9_support;

use gpui::TestAppContext;
use leon_app::palette::{filter, Item};
use m9_support::*;

fn item(name: &str, title: &str, keywords: &[&str]) -> Item {
    Item {
        name: name.into(),
        title: title.into(),
        keywords: keywords.iter().map(|k| k.to_string()).collect(),
        shortcut: None,
    }
}

fn titles(items: &[Item], query: &str) -> Vec<String> {
    filter(items, query).into_iter().map(|i| items[i].title.clone()).collect()
}

#[test]
fn filter_matches_subsequences_and_ranks_tight_matches_first() {
    let items = [
        item("pane.split.right", "Split Right", &[]),
        item("pane.split.down", "Split Below", &[]),
        item("client.sidebar.toggle", "Toggle Sidebar", &[]),
        item("session.new", "New Session", &[]),
        item("client.tab.new", "New Tab", &[]),
        item("deploy", "Deploy the site", &["ship", "release"]),
    ];
    assert_eq!(filter(&items, "").len(), items.len(), "an empty query lists everything");
    assert_eq!(titles(&items, "split r"), ["Split Right"]);
    assert_eq!(titles(&items, "zzz"), Vec::<String>::new(), "no match, no rows");
    assert_eq!(titles(&items, "SIDEBAR"), ["Toggle Sidebar"], "case-insensitive");
    assert_eq!(titles(&items, "ship"), ["Deploy the site"], "keywords match");
    assert_eq!(titles(&items, "tab.new"), ["New Tab"], "names match");
    // "nt" is New Tab's word starts, and only scattered in the others
    let nt = titles(&items, "nt");
    assert_eq!(nt.first().map(String::as_str), Some("New Tab"), "word starts rank first: {nt:?}");
    let ses = titles(&items, "ses");
    assert_eq!(ses.first().map(String::as_str), Some("New Session"), "a contiguous run ranks first: {ses:?}");
}

#[gpui::test]
fn the_palette_opens_filters_runs_and_closes(cx: &mut TestAppContext) {
    let s = setup_with_config(
        r#"leon.action{ name = "say_hi", title = "Say Hi", category = "Demo", keywords = {"greet"},
             run = function(ctx) leon.session.set_label{ label = "hi from lua" } end }"#,
    );
    let (ws, cx) = open(cx, &s.app);
    ws.update(cx, |ws, _| ws.sync_keymap());
    assert!(!ws.read_with(cx, |ws, _| ws.palette_open()));
    cx.simulate_keystrokes("cmd-shift-p");
    assert!(ws.read_with(cx, |ws, _| ws.palette_open()), "cmd-shift-p opens the palette");

    let rows = ws.read_with(cx, |ws, _| ws.palette_rows());
    for want in [
        "New Session",
        "New Tab",
        "Split Right",
        "Split Below",
        "Close Pane",
        "Zoom Pane",
        "Focus Left",
        "Focus Right",
        "Focus Up",
        "Focus Down",
        "Next Tab",
        "Previous Tab",
        "Toggle Sidebar",
        "Increase Font Size",
        "Decrease Font Size",
        "Reset Font Size",
        "Reload Config",
        "Say Hi",
    ] {
        assert!(rows.iter().any(|(t, _)| t == want), "the palette lists {want}: {rows:?}");
    }
    let shortcut = |t: &str| rows.iter().find(|(x, _)| x == t).and_then(|(_, k)| k.clone());
    assert_eq!(shortcut("Split Right").as_deref(), Some("⌘D"), "rows show their shortcut");
    assert_eq!(shortcut("Increase Font Size").as_deref(), Some("⌘="));

    cx.simulate_input("split right");
    let rows = ws.read_with(cx, |ws, _| ws.palette_rows());
    assert_eq!(rows.first().map(|(t, _)| t.as_str()), Some("Split Right"), "{rows:?}");
    cx.simulate_keystrokes("enter");
    assert!(!ws.read_with(cx, |ws, _| ws.palette_open()), "enter runs and closes");
    assert_eq!(leaves(&window(&s.app, &ws, cx).0).len(), 2, "Split Right ran");

    // escape closes without running anything
    cx.simulate_keystrokes("cmd-shift-p");
    cx.simulate_input("new tab");
    cx.simulate_keystrokes("escape");
    assert!(!ws.read_with(cx, |ws, _| ws.palette_open()));
    assert_eq!(window(&s.app, &ws, cx).1, 1, "escape ran nothing");

    // the palette's keys never reach the pane
    let t = focused(&s.app, &ws, cx);
    run_until(&t, "echo typed-ok", "typed-ok");
    cx.simulate_keystrokes("cmd-shift-p");
    cx.simulate_input("greet");
    let rows = ws.read_with(cx, |ws, _| ws.palette_rows());
    assert_eq!(rows.first().map(|(t, _)| t.as_str()), Some("Say Hi"), "config actions are listed: {rows:?}");
    cx.simulate_keystrokes("down up enter");
    let sid = ws.read_with(cx, |ws, _| ws.session_id()).unwrap();
    let label = || s.app.ctx().state().session(&sid).unwrap().label.clone();
    assert!(eventually(|| label().as_deref() == Some("hi from lua")), "the config action ran: {:?}", label());
    assert!(!t.text(true).contains("greet"), "typing in the palette didn't reach the shell");
    s.app.stop();
}
