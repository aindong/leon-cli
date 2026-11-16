//! M9 criterion 6: cmd+= / cmd+- / cmd+0 change the font size of every pane, and the PTYs follow:
//! the panes on screen at once, a hidden tab when it is shown.

mod m9_support;

use gpui::TestAppContext;
use m9_support::*;

#[gpui::test]
fn font_size_keys_resize_every_pane(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    cx.simulate_keystrokes("cmd-d");
    cx.run_until_parked();
    let ids = leaves(&window(&s.app, &ws, cx).0);
    let sizes = || ids.iter().map(|id| term(&s.app, id).size()).collect::<Vec<_>>();
    let start = sizes();
    let start_pt = ws.read_with(cx, |ws, _| ws.font_size());
    // a hidden tab, never drawn at this font size
    let sid = ws.read_with(cx, |ws, _| ws.session_id()).unwrap();
    let tab = s.app.call("session.new_window", serde_json::json!({ "session_id": sid })).unwrap();
    let hidden = term(&s.app, tab["block_id"].as_str().unwrap());
    cx.simulate_keystrokes("cmd-1");
    cx.run_until_parked();
    let h = hidden.size();

    cx.simulate_keystrokes("cmd-=");
    cx.run_until_parked();
    assert!(ws.read_with(cx, |ws, _| ws.font_size()) > start_pt);
    let bigger = sizes();
    for (b, a) in bigger.iter().zip(&start) {
        assert!(b.0 < a.0 && b.1 < a.1, "a bigger font gives fewer rows and columns: {a:?} -> {b:?}");
    }
    // the hidden tab keeps its size until it is shown
    assert!(stays(cx, &hidden, h), "a hidden tab keeps its size through a font change");
    cx.simulate_keystrokes("cmd-2");
    assert!(shows(cx, |_| hidden.size() != h), "shown, the tab fits the font");
    cx.simulate_keystrokes("cmd-1");
    cx.run_until_parked();

    cx.simulate_keystrokes("cmd-0 cmd--");
    cx.run_until_parked();
    for (s2, a) in sizes().iter().zip(&start) {
        assert!(s2.0 > a.0 && s2.1 > a.1, "a smaller font gives more: {a:?} -> {s2:?}");
    }

    cx.simulate_keystrokes("cmd-0");
    cx.run_until_parked();
    assert_eq!(ws.read_with(cx, |ws, _| ws.font_size()), start_pt);
    assert_eq!(sizes(), start, "cmd-0 restores the size");

    // the program sees it too
    cx.simulate_keystrokes("cmd-=");
    cx.run_until_parked();
    let t = term(&s.app, &ids[1]);
    let (rows, cols) = t.size();
    run_until(&t, "stty size", &format!("{rows} {cols}"));
    s.app.stop();
}
