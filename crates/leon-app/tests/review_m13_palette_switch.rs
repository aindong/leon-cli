//! Review of M13: ⌘K with the command palette open turns it into the switcher (and ⌘⇧P back),
//! rather than closing it; the key of the list on screen closes it.

mod m9_support;

use gpui::TestAppContext;
use m9_support::*;

#[gpui::test]
fn each_lists_key_switches_to_it_and_closes_it(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    cx.simulate_keystrokes("cmd-shift-p");
    assert!(ws.read_with(cx, |ws, _| ws.palette_open() && !ws.switcher_open()));
    cx.simulate_keystrokes("cmd-k");
    assert!(ws.read_with(cx, |ws, _| ws.switcher_open()), "cmd+k in the palette opens the switcher");
    cx.simulate_keystrokes("cmd-shift-p");
    assert!(ws.read_with(cx, |ws, _| ws.palette_open() && !ws.switcher_open()), "and cmd+shift+p goes back");
    cx.simulate_keystrokes("cmd-shift-p");
    assert!(!ws.read_with(cx, |ws, _| ws.palette_open()), "its own key closes it");
    s.app.stop();
}
