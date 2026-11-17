//! init.lua's font size changing fits the shown pane to it and leaves a pane of a hidden session as it was.
mod m9_support;
use gpui::TestAppContext;
use m9_support::*;
use serde_json::json;

#[gpui::test]
fn a_font_from_init_lua_fits_only_the_shown_panes(cx: &mut TestAppContext) {
    let s = setup_with_config("leon.set(\"font.size\", 13)\n");
    let (ws, cx) = open(cx, &s.app);
    ws.update(cx, |ws, cx| ws.sync_keymap(cx));
    let shown = focused(&s.app, &ws, cx);
    let (_, block) = hidden_session(&s.app, &ws, cx);
    let hidden = term(&s.app, &block);
    assert!(shows(cx, |_| shown.size() != (24, 80)), "the shown pane was fitted");
    let before = shown.size();

    config_set(&s, "font.size", json!(15));
    ws.update(cx, |ws, cx| ws.sync_keymap(cx));
    let fitted = shows(cx, |_| shown.size() != before);
    let got = shown.size();
    let kept = stays(cx, &hidden, (24, 80));
    s.app.stop();
    assert!(fitted && got.0 < before.0 && got.1 < before.1, "the shown pane fits 15 pt: {before:?} -> {got:?}");
    assert!(kept, "a pane of a hidden session keeps its size: {:?}", hidden.size());
}
