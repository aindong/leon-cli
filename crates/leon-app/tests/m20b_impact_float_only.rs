//! M20b impact: a window whose every block floats has no tiled tree. It must still draw its float
//! (not the "no session" empty state), take keys there, and size it.

mod m20b_support;

use gpui::TestAppContext;
use leon_app::embedded;
use m20b_support::*;

#[gpui::test]
fn a_window_with_only_a_float_draws_it_and_types_into_it(cx: &mut TestAppContext) {
    let dir = private_dir();
    let app = std::rc::Rc::new(embedded::start_at(dir.path().join("server.sock")).unwrap());
    let (ws, cx) = open_with(cx, &app, None);
    let sid = sid(&ws, cx);
    let id = first_tile(&app, &sid);
    act(&ws, cx, "pane.float");
    assert!(eventually(|| view_window(&app, &sid)["layout"].is_null()), "no tiled tree is left");
    assert!(shows(cx, |cx| drawn(cx, &format!("float-{id}"))), "the float is drawn");
    assert!(!drawn(cx, "empty-state"), "not the empty state");
    assert_eq!(ws.read_with(cx, |ws, _| ws.target_block()), Some(id.clone()), "keys go to it");
    let t = term(&app, &id);
    settle(&t);
    cx.simulate_input("echo float-$((40+2))");
    cx.simulate_keystrokes("enter");
    assert!(eventually(|| has_line(&t, "float-42")), "typed into the float:\n{}", t.text(true));
    app.stop();
}
