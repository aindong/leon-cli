//! M20b criterion 7: dragging a float's header moves it, dragging its right or bottom edge resizes
//! it, each step sent to the server, so `session.view` (the CLI, attach) has the new bounds.

mod m20b_support;

use gpui::{point, px, Modifiers, MouseButton, TestAppContext};
use leon_app::embedded;
use m20b_support::*;

#[gpui::test]
fn header_and_edge_drags_follow_the_pointer(cx: &mut TestAppContext) {
    let dir = private_dir();
    let app = std::rc::Rc::new(embedded::start_at(dir.path().join("server.sock")).unwrap());
    let (ws, cx) = open_with(cx, &app, None);
    cx.simulate_keystrokes("cmd-d");
    cx.run_until_parked();
    let sid = sid(&ws, cx);
    let id = new_float(&app, &sid, [0.1, 0.1, 0.4, 0.4]);
    assert!(shows(cx, |cx| drawn(cx, &format!("header-{id}"))));
    let area = at(cx, "panes-area").unwrap();
    let (w, h) = (f32::from(area.size.width) as f64, f32::from(area.size.height) as f64);

    // the header: its left part, away from the controls
    let header = at(cx, &format!("header-{id}")).unwrap();
    let from = point(header.origin.x + px(40.), header.origin.y + header.size.height / 2.);
    cx.simulate_mouse_down(from, MouseButton::Left, Modifiers::none());
    for step in 1..=20 {
        let (dx, dy) = (5. * step as f32, 3. * step as f32);
        cx.simulate_mouse_move(from + point(px(dx), px(dy)), MouseButton::Left, Modifiers::none());
        cx.run_until_parked();
        let got = float_bounds(&app, &sid, &id).unwrap();
        let want = [0.1 + dx as f64 / w, 0.1 + dy as f64 / h];
        assert!(near(got[0], want[0], 0.5 / w) && near(got[1], want[1], 0.5 / h), "step {step}: {got:?} vs {want:?}");
        assert!(near(got[2], 0.4, 1e-9) && near(got[3], 0.4, 1e-9), "a move keeps the size: {got:?}");
    }
    cx.simulate_mouse_up(from + point(px(100.), px(60.)), MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    let moved = float_bounds(&app, &sid, &id).unwrap();

    // the right edge
    let edge = at(cx, &format!("float-edge-right-{id}")).expect("a right edge handle");
    let from = center(edge);
    cx.simulate_mouse_down(from, MouseButton::Left, Modifiers::none());
    for step in 1..=10 {
        let dx = 8. * step as f32;
        cx.simulate_mouse_move(from + point(px(dx), px(0.)), MouseButton::Left, Modifiers::none());
        cx.run_until_parked();
        let got = float_bounds(&app, &sid, &id).unwrap();
        assert!(near(got[2], moved[2] + dx as f64 / w, 0.5 / w), "step {step}: width {got:?}");
        assert!(near(got[0], moved[0], 1e-9), "the left edge stays");
    }
    cx.simulate_mouse_up(from, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    let wider = float_bounds(&app, &sid, &id).unwrap();

    // the bottom edge
    let edge = at(cx, &format!("float-edge-bottom-{id}")).expect("a bottom edge handle");
    let from = center(edge);
    cx.simulate_mouse_down(from, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(from + point(px(0.), px(50.)), MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_up(from + point(px(0.), px(50.)), MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    let got = float_bounds(&app, &sid, &id).unwrap();
    assert!(near(got[3], wider[3] + 50. / h, 0.5 / h), "the height follows: {got:?}");
    assert!(near(got[1], wider[1], 1e-9) && near(got[2], wider[2], 1e-9), "{got:?}");

    // and the app draws it where the server has it
    let r = at(cx, &format!("float-{id}")).unwrap();
    let x = f32::from(area.origin.x) as f64 + got[0] * w;
    assert!((f32::from(r.origin.x) as f64 - x).abs() <= 1., "{r:?} vs x {x}");
    app.stop();
}
