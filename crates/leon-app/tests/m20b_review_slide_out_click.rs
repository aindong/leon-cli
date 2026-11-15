//! Review (correctness): a click on a drawer that is sliding out goes to the tile under it and
//! doesn't show the drawer again.

mod m20b_support;

use gpui::{Modifiers, MouseButton, TestAppContext};
use leon_app::embedded;
use m20b_support::*;
use serde_json::json;

#[gpui::test]
fn a_click_on_a_drawer_sliding_out_leaves_it_hidden(cx: &mut TestAppContext) {
    let dir = private_dir();
    let app = std::rc::Rc::new(embedded::start_at(dir.path().join("server.sock")).unwrap());
    let (ws, cx) = open_with(cx, &app, None);
    ws.update(cx, |ws, _| ws.set_reduce_motion(false));
    let sid = sid(&ws, cx);
    let tile = first_tile(&app, &sid);
    app.call("session.toggle", json!({ "session_id": sid, "label": "scratch", "create": true })).unwrap();
    let id = view_window(&app, &sid)["docked"][0]["block_id"].as_str().unwrap().to_string();
    assert!(shows(cx, |cx| drawn(cx, &format!("dock-{id}"))));
    std::thread::sleep(std::time::Duration::from_millis(200));
    ws.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();

    app.call("session.toggle", json!({ "session_id": sid, "label": "scratch" })).unwrap();
    let b = in_slide_frame(&ws, cx, &format!("dock-{id}")).expect("still drawn while it slides out");
    // its part still on screen: the top strip (half slid out, its centre is past the window's edge)
    let p = gpui::point(b.origin.x + b.size.width / 2., b.origin.y + gpui::px(10.));
    cx.simulate_mouse_down(p, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_up(p, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
    let w = view_window(&app, &sid);
    app.stop();
    assert!(w["docked"][0]["hidden"] == true, "clicking a drawer as it slides out showed it again");
    assert_eq!(w["focused_block_id"].as_str(), Some(tile.as_str()), "the click went to the tile under it");
}
