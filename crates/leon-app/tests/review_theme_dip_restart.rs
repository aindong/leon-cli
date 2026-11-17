//! Review finding 2 (.agent/review/findings.md): a theme change that starts mid-way through
//! another keeps the panes dipped instead of flashing their text back to full strength.

mod m10b_support;

use gpui::TestAppContext;
use leon_app::floats::SLIDE;
use m10b_support::*;

#[gpui::test]
fn a_change_mid_way_keeps_the_dip(cx: &mut TestAppContext) {
    let s = setup_with_config("-- mine\n");
    let (ws, cx) = open(cx, &s.app);
    ws.update(cx, |ws, cx| ws.sync_keymap(cx));
    let pane = ws.read_with(cx, |ws, _| ws.target_block()).expect("a pane");
    let t0 = cx.executor().now(); // GPUI's clock, which the motions read
    ws.update(cx, |ws, _| ws.set_reduce_motion(false));
    act(&ws, cx, "client.theme.change");
    cx.simulate_keystrokes("down");
    // half way through on GPUI's clock, the clock the motions read
    cx.executor().advance_clock((t0 + SLIDE / 2).saturating_duration_since(cx.executor().now()));
    ws.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();
    let before = ws.read_with(cx, |ws, _| ws.pane_fade(&pane));
    cx.simulate_keystrokes("down"); // the next theme, at the same instant
    let after = ws.read_with(cx, |ws, _| ws.pane_fade(&pane));
    s.app.stop();
    assert!(before > 0.5, "mid-way the pane is dipped: {before}");
    assert!(after >= before - 0.1, "a second change mid-way jumps the dip from {before} to {after}");
}
