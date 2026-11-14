//! M10b criterion 10: with no sessions the app teaches: new session, the palette, and agents.

mod m10b_support;

use gpui::TestAppContext;
use m10b_support::*;

#[gpui::test]
fn the_empty_state_lists_the_keys_that_matter(cx: &mut TestAppContext) {
    let s = setup_with_config(r#"leon.bind("cmd+k", "client.palette.toggle")"#);
    let (ws, cx) = open(cx, &s.app);
    ws.update(cx, |ws, _| ws.sync_keymap());
    cx.simulate_keystrokes("cmd-w");
    assert!(s.app.ctx().state().sessions.is_empty());
    assert!(drawn(cx, "empty-state"), "an empty state, not a blank pane area");
    let hints = ws.read_with(cx, |ws, _| ws.empty_hints());
    let keys: Vec<&str> = hints.iter().map(|(k, _)| k.as_str()).collect();
    assert_eq!(keys, ["⌘N", "⌘K", "leon skill setup"], "from the live keymap: {hints:?}");
    s.app.stop();
}
