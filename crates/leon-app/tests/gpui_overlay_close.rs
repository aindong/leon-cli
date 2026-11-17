//! Motion PR 3: a closed palette stops taking keys at once and fades out from what it last showed,
//! then is gone.
mod layout_support;

use gpui::TestAppContext;
use layout_support::*;
use leon_app::overlay_motion::{Look, OVERLAY_OUT};

#[gpui::test]
fn a_closed_palette_fades_out_then_goes(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    ws.update(cx, |ws, _| ws.set_reduce_motion(false));
    act(&ws, cx, "client.palette.toggle");
    to_rest(&ws, cx);
    let look = |cx: &mut gpui::VisualTestContext| ws.read_with(cx, |ws, _| ws.overlay_look("palette"));
    assert_eq!(look(cx), Some(Look::SHOWN));
    cx.simulate_keystrokes("escape");
    let open = ws.read_with(cx, |ws, _| ws.palette_open());
    let drawn_going = drawn(cx, "palette");
    frame(&ws, cx, OVERLAY_OUT / 2);
    let half = look(cx);
    to_rest(&ws, cx);
    let gone = (look(cx), drawn(cx, "palette"), ws.read_with(cx, |ws, _| ws.frame_asked()));
    s.app.stop();
    assert!(!open, "keys stop going to it at once");
    assert!(drawn_going, "but it is still drawn as it goes");
    assert!(half.is_some_and(|l| l.opacity < 1. && l.opacity > 0.), "half way it is fading: {half:?}");
    assert_eq!(gone, (None, false, false), "then it is gone, and no frame is asked for");
}
