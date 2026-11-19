//! CodeRabbit on #94: a key the find bar leaves (a ctrl chord) goes on to the workspace with the
//! repeat window still open, so a repeatable binding repeats while the find bar has the focus.
mod m9_support;

use gpui::TestAppContext;
use m9_support::*;

const CONFIG: &str = r#"
leon.set("repeat_time", 2000)
local runs = 0
leon.bind("ctrl+b>ctrl+k", function(ctx)
  runs = runs + 1
  leon.session.set_label{ label = "runs " .. runs }
  return true
end, nil, { repeats = true })
"#;

#[gpui::test]
fn a_repeatable_chord_repeats_with_the_find_bar_open(cx: &mut TestAppContext) {
    let s = plain_setup_with_config(CONFIG);
    let (ws, cx) = open(cx, &s.app);
    ws.update(cx, |ws, cx| ws.sync_keymap(cx));
    let sid = ws.read_with(cx, |ws, _| ws.session_id()).unwrap();
    cx.simulate_keystrokes("cmd-f");
    cx.run_until_parked();
    let finding = cx.update(|window, cx| ws.read(cx).focus_owner(window, cx));
    cx.simulate_keystrokes("ctrl-b ctrl-k ctrl-k");
    let label = || s.app.ctx().state().session(&sid).ok().and_then(|s| s.label.clone()).unwrap_or_default();
    let twice = shows(cx, |_| label() == "runs 2");
    let got = label();
    s.app.stop();
    assert_eq!(finding, Some("find"), "the find bar has the focus");
    assert!(twice, "⌃K ran twice, the second time repeated without the prefix: {got:?}");
}
