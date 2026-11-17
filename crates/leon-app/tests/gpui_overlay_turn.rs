//! Motion PR 3: the palette opened again while it fades out turns back from where it is: one
//! palette, never fainter than the going one was.
mod layout_support;

use gpui::TestAppContext;
use layout_support::*;
use leon_app::overlay_motion::OVERLAY_OUT;

#[gpui::test]
fn a_palette_reopened_mid_fade_turns_back(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    ws.update(cx, |ws, _| ws.set_reduce_motion(false));
    act(&ws, cx, "client.palette.toggle");
    to_rest(&ws, cx);
    cx.simulate_keystrokes("escape");
    frame(&ws, cx, OVERLAY_OUT / 2);
    let going = ws.read_with(cx, |ws, _| ws.overlay_look("palette")).expect("fading out").opacity;
    act(&ws, cx, "client.palette.toggle");
    let back = ws.read_with(cx, |ws, _| (ws.overlay_look("palette"), ws.palette_open(), ws.palette_going()));
    to_rest(&ws, cx);
    let rest = ws.read_with(cx, |ws, _| ws.overlay_look("palette").map(|l| l.opacity));
    s.app.stop();
    let (look, open, going_too) = back;
    assert!(open && !going_too, "one palette: open, none going");
    assert!(look.is_some_and(|l| l.opacity >= going - 0.01), "it turns from where it was ({going}): {look:?}");
    assert_eq!(rest, Some(1.), "and ends shown");
}
