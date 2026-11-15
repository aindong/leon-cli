//! M20b criterion 6: a click focuses what is on top there (a float over a tile, the upper of two
//! floats), a click on the lower float raises it, and the focus keys still leave a float.

mod m20b_support;

use gpui::{point, px, Modifiers, MouseButton, TestAppContext};
use leon_app::embedded;
use m20b_support::*;

#[gpui::test]
fn clicks_focus_and_raise_by_z_order(cx: &mut TestAppContext) {
    let dir = private_dir();
    let app = std::rc::Rc::new(embedded::start_at(dir.path().join("server.sock")).unwrap());
    let (ws, cx) = open_with(cx, &app, None);
    cx.simulate_keystrokes("cmd-d");
    cx.run_until_parked();
    let sid = sid(&ws, cx);
    let tile = first_tile(&app, &sid);
    let low = new_float(&app, &sid, [0.05, 0.05, 0.5, 0.5]);
    let top = new_float(&app, &sid, [0.3, 0.3, 0.5, 0.5]);
    assert!(shows(cx, |cx| drawn(cx, &format!("float-{top}")) && drawn(cx, &format!("float-{low}"))));
    let area = at(cx, "panes-area").unwrap();
    let pt = |fx: f32, fy: f32| point(area.origin.x + area.size.width * fx, area.origin.y + area.size.height * fy);
    let click = |cx: &mut gpui::VisualTestContext, p| {
        cx.simulate_mouse_down(p, MouseButton::Left, Modifiers::none());
        cx.simulate_mouse_up(p, MouseButton::Left, Modifiers::none());
        cx.run_until_parked();
    };

    // the tile's own spot first, so the next clicks are changes
    click(cx, pt(0.97, 0.97));
    assert!(eventually(|| focused_id(&app, &sid).as_deref() != Some(top.as_str())), "a bare tile takes the focus");

    // both floats and a tile are under (0.4, 0.4): the top float gets it
    click(cx, pt(0.4, 0.4));
    assert!(eventually(|| focused_id(&app, &sid).as_deref() == Some(top.as_str())), "the top float is focused");
    assert_eq!(float_order(&app, &sid), [low.clone(), top.clone()], "the order is unchanged");

    // only the lower float is under (0.1, 0.1) (a tile is too): it is focused and raised
    click(cx, pt(0.1, 0.1) + point(px(0.), px(30.)));
    assert!(eventually(|| focused_id(&app, &sid).as_deref() == Some(low.as_str())), "the lower float is focused");
    assert!(eventually(|| float_order(&app, &sid) == [top.clone(), low.clone()]), "and raised to the top");

    // the focus keys leave the float for the tiles
    cx.simulate_keystrokes("cmd-alt-left");
    assert!(eventually(|| focused_id(&app, &sid).as_deref() == Some(tile.as_str())), "back on a tile");
    app.stop();
}
