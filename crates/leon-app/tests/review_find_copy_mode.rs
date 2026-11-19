//! Review proof (skeptic, correctness): copy mode entered while the find bar is open on its pane
//! still takes its keys, as when `key_down` ran the pane's modes before the find bar.
mod focus_support;

use focus_support::*;
use gpui::TestAppContext;

#[gpui::test]
fn copy_mode_keeps_its_keys_with_the_find_bar_open(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let id = tab_running(&s, &ws, cx, "seq 1 50; echo 'alpha beta'; exec cat", "alpha beta");
    cx.simulate_keystrokes("cmd-f");
    cx.run_until_parked();
    cx.simulate_keystrokes("cmd-shift-c");
    cx.run_until_parked();
    let before = ws.read_with(cx, |ws, _| ws.copy_cursor(&id)).expect("copy mode on").0.line;
    cx.simulate_keystrokes("k");
    cx.run_until_parked();
    let after = ws.read_with(cx, |ws, _| ws.copy_cursor(&id)).map(|c| c.0.line);
    let query = ws.read_with(cx, |ws, _| ws.find_query());
    s.app.stop();
    assert_eq!(after, Some(before - 1), "k is copy mode's, not the find bar's (query {query:?})");
}
