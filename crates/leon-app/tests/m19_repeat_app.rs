//! M19 criterion 3 (.agent/plans/2026-11-15-m19-tmux-migrants.md): repeatable bindings in the app:
//! ⌃B H H moves focus twice, and a repeated Lua function runs on the server each time and types
//! nothing.
mod m9_support;

use gpui::TestAppContext;
use m9_support::*;
use serde_json::json;

const CONFIG: &str = r#"
leon.bind("ctrl+b>h", "pane.focus.left", nil, { repeats = true })
leon.bind("ctrl+b>l", "pane.focus.right", nil, { repeats = true })
leon.set("repeat_time", 2000)
local runs = 0
leon.bind("ctrl+b>k", function(ctx)
  runs = runs + 1
  leon.session.set_label{ label = "runs " .. runs }
  return true
end, nil, { repeats = true })
"#;

#[gpui::test]
fn prefix_then_the_key_again(cx: &mut TestAppContext) {
    let s = setup_with_config(CONFIG);
    let km = s.app.call("client.keymap", json!({})).unwrap();
    assert_eq!(km["settings"]["repeat_time"], 2000);
    let h = km["bindings"].as_array().unwrap().iter().find(|b| b["key"] == "ctrl+b>h").unwrap();
    assert_eq!(h["repeats"], true, "client.keymap carries repeats: {h}");

    let (ws, cx) = open(cx, &s.app);
    ws.update(cx, |ws, cx| ws.sync_keymap(cx));
    let sid = ws.read_with(cx, |ws, _| ws.session_id()).unwrap();
    for _ in 0..2 {
        cx.simulate_keystrokes("cmd-d");
        cx.run_until_parked();
    }
    let (w, _) = window(&s.app, &ws, cx);
    let panes = leaves(&w);
    assert_eq!(panes.len(), 3);
    assert_eq!(w.focus.as_deref(), Some(panes[2].as_str()), "the new pane on the right has the focus");

    cx.simulate_keystrokes("ctrl-b h h");
    let left = shows(cx, |cx| window(&s.app, &ws, cx).0.focus.as_deref() == Some(panes[0].as_str()));
    assert!(left, "⌃B H H went two panes left: {:?}", window(&s.app, &ws, cx).0.focus);
    cx.simulate_keystrokes("l");
    let mid = shows(cx, |cx| window(&s.app, &ws, cx).0.focus.as_deref() == Some(panes[1].as_str()));
    assert!(mid, "l repeats too, still without the prefix");

    let before = term(&s.app, &panes[1]).text(false);
    cx.simulate_keystrokes("ctrl-b k k k");
    let label = |cx: &mut gpui::VisualTestContext| {
        let _ = cx;
        s.app.ctx().state().session(&sid).ok().and_then(|s| s.label.clone()).unwrap_or_default()
    };
    assert!(shows(cx, |cx| label(cx) == "runs 3"), "the function ran three times: {:?}", label(cx));
    assert_eq!(term(&s.app, &panes[1]).text(false), before, "and nothing was typed into the pane");
    s.app.stop();
}
