//! A pane header's controls (split, zoom, close; unfloat and close on a float) sit at the header's
//! right end, whatever the title, caption or find bar beside them.

mod m20b_support;

use gpui::{Bounds, Pixels, TestAppContext, VisualTestContext};
use m20b_support::*;

/// How far pane `id`'s close control ends from the right of `pane` (its tile or float).
fn gap(cx: &mut VisualTestContext, id: &str, pane: &str) -> f32 {
    let pane: Bounds<Pixels> = at(cx, pane).unwrap_or_else(|| panic!("{pane} drawn"));
    let close = at(cx, &format!("header-close-{id}")).unwrap_or_else(|| panic!("{id}'s close drawn"));
    f32::from(pane.right() - close.right())
}

#[gpui::test]
fn the_controls_end_at_the_headers_right(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let id = ws.read_with(cx, |ws, _| ws.target_block()).unwrap();
    let tile = format!("pane-{id}");
    let short = gap(cx, &id, &tile);
    assert!((0. ..12.).contains(&short), "a short title: the close control ends {short}px from the right");

    let t = term(&s.app, &id);
    let long = "a-title-long-enough-to-be-cut-by-the-header-".repeat(4);
    // exec: no prompt comes back to set its own title
    t.write(format!("printf '\\033]2;{long}\\007'; exec sleep 300\r").as_bytes()).unwrap();
    let cut = |cx: &mut VisualTestContext| {
        at(cx, &format!("header-title-{id}")).is_some_and(|b| b.size.width > gpui::px(250.))
    };
    assert!(shows(cx, cut), "the long title is drawn");
    let long_gap = gap(cx, &id, &tile);
    assert!((0. ..12.).contains(&long_gap), "a long cut title: {long_gap}px");

    act(&ws, cx, "client.find.open");
    assert!(drawn(cx, &format!("find-bar-{id}")), "the find bar is open");
    let find = gap(cx, &id, &tile);
    assert!((0. ..12.).contains(&find), "with the find bar open: {find}px");
    cx.simulate_keystrokes("escape");

    let sid = sid(&ws, cx);
    let float = new_float(&s.app, &sid, [0.1, 0.1, 0.6, 0.5]);
    assert!(shows(cx, |cx| drawn(cx, &format!("header-close-{float}"))), "the float's controls show");
    let on_float = gap(cx, &float, &format!("float-{float}"));
    assert!((0. ..12.).contains(&on_float), "a float: {on_float}px");
    s.app.stop();
}
