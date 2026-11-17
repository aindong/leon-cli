//! Review finding 3 (.agent/review/findings.md): an overview card is drawn in the theme a change
//! heads for, not in the colours the window has mid-way, which nothing would refresh once the
//! window's fade ends.

mod m10b_support;

use std::time::{Duration, Instant};

use gpui::{TestAppContext, VisualTestContext};
use leon_app::theme::{DARK, LIGHT};
use m10b_support::*;

/// `shows`, with the test clock moving too: a card's refresh `THUMB_EVERY` held back is a timer on it.
fn ticks(cx: &mut VisualTestContext, mut pred: impl FnMut(&mut VisualTestContext) -> bool) -> bool {
    let end = Instant::now() + Duration::from_secs(10);
    loop {
        if pred(cx) {
            return true;
        }
        if Instant::now() >= end {
            return false;
        }
        cx.executor().advance_clock(Duration::from_millis(50));
        cx.run_until_parked();
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[gpui::test]
fn a_card_is_drawn_in_the_theme_a_change_heads_for(cx: &mut TestAppContext) {
    let s = setup_with_config("leon.set(\"theme\", { dark = \"leon-dark\", light = \"leon-light\" })\n");
    let (ws, cx) = open(cx, &s.app);
    ws.update(cx, |ws, _| ws.sync_keymap());
    let (w, _) = window(&s.app, &ws, cx);
    let id = leaves(&w)[0].clone();
    assert!(shows(cx, |cx| ws.read_with(cx, |ws, _| ws.screen_text(&id).is_some())));
    act(&ws, cx, "client.overview.toggle");
    let card = |cx: &mut VisualTestContext| ws.read_with(cx, |ws, _| ws.thumb_pane(&w.id, &id).map(|(_, t)| t));
    assert!(ticks(cx, |cx| card(cx) == Some(LIGHT)), "the card is drawn in leon-light");
    // the window's change held at its start: its colours stay leon-light's
    let t0 = Instant::now();
    ws.update(cx, |ws, _| {
        ws.set_reduce_motion(false);
        ws.set_fade_clock(t0);
    });
    ws.update_in(cx, |ws, _, cx| ws.set_appearance(true, cx));
    let dark = ticks(cx, |cx| card(cx) == Some(DARK));
    let drawn = ws.read_with(cx, |ws, _| ws.drawn_theme());
    s.app.stop();
    assert_eq!(drawn, LIGHT, "the window is still at the start of its change");
    assert!(dark, "the card is redrawn in leon-dark, where the change ends");
}
