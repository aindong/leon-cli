//! Daily-driver A3: a pane header that can't fit its directory cuts it in the middle, where the
//! parts that differ between panes are kept (the start and the last two), and hovering the title
//! shows the whole of it. GPUI's test text system gives every character the same width (6.6 px at
//! the header's 11 px), so the room is exact here.

mod m10b_support;

use std::time::Duration;

use gpui::{Modifiers, TestAppContext};
use leon_app::chrome::elide_path;
use m10b_support::*;
use serde_json::json;

fn chars(n: usize) -> impl Fn(&str) -> bool {
    move |s: &str| s.chars().count() <= n
}

#[test]
fn a_long_path_keeps_its_start_and_its_last_two() {
    assert_eq!(elide_path("~/src/one/two/three/api/routes", chars(20)), "~/src/…/api/routes");
    assert_eq!(elide_path("/tmp/a-long-scratch-dir/x/y/z", chars(20)), "/tmp/…/y/z");
    assert_eq!(elide_path("~/src/api/routes", chars(10)), "~/src/api/routes", "nothing between to cut");
    assert_eq!(elide_path("~/src/one/two/three/api/routes", chars(40)), "~/src/one/two/three/api/routes", "it fits");
}

#[gpui::test]
fn a_header_too_narrow_for_its_path_cuts_the_middle_and_tells_the_rest(cx: &mut TestAppContext) {
    let s = setup();
    let deep = s.dir.path().join("a-long-scratch-dir/one/two/api/routes");
    std::fs::create_dir_all(&deep).unwrap();
    let want = real(&deep);
    // sh sets no title of its own (a login zsh may), so the header names the program and its dir
    let pane = json!({ "block": { "options": { "command": ["/bin/sh"], "cwd": want } } });
    s.app.call("session.create", json!({ "layout": pane })).unwrap();
    let (ws, cx) = open(cx, &s.app);
    let t = focused(&s.app, &ws, cx);
    let id = ws.read_with(cx, |ws, _| ws.target_block()).unwrap();
    let full = shows(cx, |cx| ws.read_with(cx, |ws, _| ws.pane_title(&id)).is_some_and(|t| t.ends_with(&want)));
    assert!(full, "{:?}", ws.read_with(cx, |ws, _| ws.pane_title(&id)));
    let title = ws.read_with(cx, |ws, _| ws.pane_title(&id)).unwrap();
    let program = title.split(' ').next().unwrap().to_string();

    // 200 px holds 30 characters: not the whole title, but its cut form
    let (shown, tip) = ws.read_with(cx, |ws, cx| ws.header_title(&id, 200., cx));
    assert!(shown.starts_with(&format!("{program} /")) && shown.ends_with("/…/api/routes"), "{shown}");
    assert_eq!(tip.as_deref(), Some(title.as_str()), "the tooltip has the whole title");
    let (shown, tip) = ws.read_with(cx, |ws, cx| ws.header_title(&id, 2000., cx));
    assert_eq!((shown.as_str(), tip), (title.as_str(), None), "room for it all: unchanged");

    // drawn: three panes side by side leave each title too little room, so hovering shows it all
    cx.simulate_keystrokes("cmd-d");
    cx.simulate_keystrokes("cmd-d");
    let first = at(cx, &format!("header-title-{id}")).expect("the first pane's title is drawn");
    cx.simulate_mouse_move(center(first), None, Modifiers::none());
    cx.executor().advance_clock(Duration::from_secs(1));
    assert!(drawn(cx, "tooltip"), "hovering a cut title shows the whole");

    run_until(&t, "cd / && echo at-root", "at-root");
    let short = shows(cx, |cx| ws.read_with(cx, |ws, _| ws.pane_title(&id)).is_some_and(|t| t.ends_with(" /")));
    assert!(short);
    let (shown, tip) = ws.read_with(cx, |ws, cx| ws.header_title(&id, 200., cx));
    assert_eq!((shown, tip), (format!("{program} /"), None), "nothing cut, no tooltip");
    s.app.stop();
}
