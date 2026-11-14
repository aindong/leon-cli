//! M9 criterion 3: up and down move the palette's selection, and enter runs the selected row.

mod m9_support;

use gpui::TestAppContext;
use m9_support::*;

#[gpui::test]
fn down_and_up_pick_the_row_enter_runs(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let size = |cx: &mut gpui::VisualTestContext| ws.read_with(cx, |ws, _| ws.font_size());
    let start = size(cx);
    cx.simulate_keystrokes("cmd-shift-p");
    cx.simulate_input("font size");
    let rows: Vec<String> = ws.read_with(cx, |ws, _| ws.palette_rows()).into_iter().map(|(t, _)| t).collect();
    assert_eq!(rows.len(), 3, "{rows:?}");
    let effect = |title: &str, now: f32| match title {
        "Increase Font Size" => now > start,
        "Decrease Font Size" => now < start,
        _ => now == start,
    };
    // the second row runs, not the first
    cx.simulate_keystrokes("down down up enter");
    assert!(!ws.read_with(cx, |ws, _| ws.palette_open()));
    let now = size(cx);
    assert!(effect(&rows[1], now), "{} should have run: {start} -> {now}", rows[1]);
    assert!(rows[0] == "Reset Font Size" || !effect(&rows[0], now), "the first row didn't run");
    s.app.stop();
}
