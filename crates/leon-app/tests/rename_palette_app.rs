//! Rename Tab… and Rename Session… in the palette: it asks, naming the current name; Enter sets
//! the typed one, an empty one clears the label, Escape changes nothing.
mod close_support;

use close_support::*;
use gpui::TestAppContext;
use serde_json::json;

#[gpui::test]
fn renaming_a_tab_and_a_session(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let sid = ws.read_with(cx, |ws, _| ws.session_id()).unwrap();
    let before = ws.read_with(cx, |ws, _| ws.tab_titles())[0].clone();
    act(&ws, cx, "client.tab.rename");
    assert!(ws.read_with(cx, |ws, _| ws.palette_open()), "the palette asks");
    let prompt = ws.read_with(cx, |ws, _| ws.palette_prompt());
    assert!(prompt.contains(&before), "it names the tab: {prompt:?}");
    cx.simulate_keystrokes("l o g s enter");
    assert!(shows(cx, |cx| ws.read_with(cx, |ws, _| ws.tab_titles()) == ["logs"]), "the tab is logs");

    act(&ws, cx, "client.tab.rename");
    cx.simulate_keystrokes("x escape");
    cx.run_until_parked();
    assert_eq!(ws.read_with(cx, |ws, _| ws.tab_titles()), ["logs"], "Escape changes nothing");
    act(&ws, cx, "client.tab.rename");
    cx.simulate_keystrokes("enter");
    // the program's own title again (whatever the shell set by now), not the label
    let cleared = shows(cx, |cx| ws.read_with(cx, |ws, _| ws.tab_titles()).first().is_some_and(|t| t != "logs"));
    assert!(cleared, "empty clears the label");

    act(&ws, cx, "session.rename");
    cx.simulate_keystrokes("w e b enter");
    let named = shows(cx, |_| label(&s, &sid) == json!("web"));
    s.app.stop();
    assert!(named, "the session is web");
}
