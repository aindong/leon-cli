//! Review proof: a divider drag made of small moves (a real drag) follows the pointer at every step.

use std::os::unix::fs::PermissionsExt;
use std::rc::Rc;

use gpui::{point, px, size, Bounds, Modifiers, MouseButton, Pixels, TestAppContext};
use leon_app::embedded;
use leon_app::view::{self, Workspace};
use leon_core::layout::Node;

fn center(b: Bounds<Pixels>) -> gpui::Point<Pixels> {
    point(b.origin.x + b.size.width / 2., b.origin.y + b.size.height / 2.)
}

#[gpui::test]
fn a_divider_follows_small_pointer_moves(cx: &mut TestAppContext) {
    let dir = tempfile::Builder::new().permissions(std::fs::Permissions::from_mode(0o700)).tempdir().unwrap();
    let app = Rc::new(embedded::start_at(dir.path().join("server.sock")).unwrap());
    cx.update(view::init);
    let a2 = app.clone();
    let (ws, cx) = cx.add_window_view(move |window, cx| {
        Workspace::new(Rc::new(leon_app::remote::Remote::connect(a2.socket()).expect("the app attaches")), window, cx)
    });
    cx.simulate_resize(size(px(1400.), px(900.)));
    cx.run_until_parked();
    cx.simulate_keystrokes("cmd-d");
    cx.run_until_parked();

    let win = |cx: &mut gpui::VisualTestContext| {
        let sid = ws.read_with(cx, |ws, _| ws.session_id()).unwrap();
        let st = app.ctx().state();
        let s = st.session(&sid).unwrap();
        s.windows[s.active].clone()
    };
    let w = win(cx);
    let ids: Vec<String> = w.layout.as_ref().unwrap().leaves().into_iter().map(String::from).collect();
    let ratio = |w: &leon_core::Window| match w.layout.as_ref().unwrap() {
        Node::Split { ratio, .. } => *ratio,
        _ => panic!("not split"),
    };
    let old = ratio(&w);
    let pa = cx.debug_bounds(Box::leak(format!("pane-{}", ids[0]).into_boxed_str())).unwrap();
    let pb = cx.debug_bounds(Box::leak(format!("pane-{}", ids[1]).into_boxed_str())).unwrap();
    let room = f32::from(pb.origin.x + pb.size.width - pa.origin.x) as f64 - 1.;

    let from = center(cx.debug_bounds("divider-0").expect("divider"));
    cx.simulate_mouse_down(from, MouseButton::Left, Modifiers::none());
    for step in 1..=50 {
        let moved = 2. * step as f32;
        cx.simulate_mouse_move(point(from.x + px(moved), from.y), MouseButton::Left, Modifiers::none());
        let got = ratio(&win(cx));
        let want = old + moved as f64 / room;
        assert!(
            (got - want).abs() * room < 0.5,
            "the divider follows the pointer: after {moved}px the ratio is {got}, wanted {want}"
        );
    }
    cx.simulate_mouse_up(point(from.x + px(100.), from.y), MouseButton::Left, Modifiers::none());
    app.stop();
}
