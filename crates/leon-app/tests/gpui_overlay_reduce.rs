//! Motion PR 3: with Reduce Motion every overlay shows and goes on its first frame; nothing fades
//! at launch.
mod layout_support;

use gpui::TestAppContext;
use layout_support::*;
use leon_app::overlay_motion::Look;

#[gpui::test]
fn reduce_motion_shows_and_hides_overlays_at_once(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let launch = ws.read_with(cx, |ws, _| (ws.overlay_look("list"), ws.frame_asked()));
    ws.update(cx, |ws, _| ws.set_reduce_motion(true));
    let state = |cx: &mut gpui::VisualTestContext, name: &str| {
        ws.read_with(cx, |ws, _| (ws.overlay_look(name), ws.frame_asked()))
    };
    act(&ws, cx, "client.palette.toggle");
    let palette = state(cx, "palette");
    cx.simulate_keystrokes("escape");
    let closed = state(cx, "palette");
    act(&ws, cx, "client.find.open");
    let find = state(cx, "find");
    cx.simulate_keystrokes("escape");
    let found = state(cx, "find");
    act(&ws, cx, "client.sidebar.activity.toggle");
    let list = state(cx, "list");
    s.app.stop();
    assert_eq!(launch, (Some(Look::SHOWN), false), "nothing fades at launch");
    assert_eq!(palette, (Some(Look::SHOWN), false), "the palette shows at once");
    assert_eq!(closed, (None, false), "and goes at once");
    assert_eq!(find, (Some(Look::SHOWN), false), "the find bar shows at once");
    assert_eq!(found, (None, false), "and goes at once");
    assert_eq!(list, (Some(Look::SHOWN), false), "the Activity list shows at once");
}
