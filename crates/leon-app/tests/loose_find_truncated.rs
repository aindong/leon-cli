//! Loose ends criterion 2 (.agent/plans/2026-11-15-loose-ends-m12-m21.md): past find's 10000
//! matches the server says `truncated`; the find bar's count and copy mode's badge say so too.
mod m11_support;

use gpui::TestAppContext;
use m11_support::*;

/// 12500 `x`s over 2500 lines, a `y` on the last, then `cat`.
const SCRIPT: &str = "stty -echo; yes 'x x x x x' | head -n 2500; echo 'y done'; exec cat";

#[gpui::test]
fn the_find_bar_and_copy_mode_say_when_matches_were_cut_off(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let id = tab_running(&s, &ws, cx, SCRIPT, "y done");

    cx.simulate_keystrokes("cmd-f x");
    cx.run_until_parked();
    // find also stops after 300 ms, so a slow build may count fewer than 10000
    let count = ws.read_with(cx, |ws, _| ws.find_count()).unwrap_or_default();
    let (n, of) = count.split_once(" of ").unwrap_or_default();
    assert_eq!(of, format!("{n}+"), "the newest found, and a + for the rest: {count:?}");
    cx.simulate_keystrokes("backspace y");
    cx.run_until_parked();
    assert_eq!(ws.read_with(cx, |ws, _| ws.find_count()).as_deref(), Some("1 of 1"), "no + when all were found");
    cx.simulate_keystrokes("escape");

    cx.simulate_keystrokes("cmd-shift-c ? x enter");
    cx.run_until_parked();
    let top = ws.read_with(cx, |ws, _| ws.drawn_text(&id)).unwrap();
    let top = top.lines().next().unwrap_or_default().trim_end().to_string();
    assert!(top.ends_with("+ matches"), "the badge: {top:?}");
    cx.simulate_keystrokes("? y enter");
    cx.run_until_parked();
    let top = ws.read_with(cx, |ws, _| ws.drawn_text(&id)).unwrap();
    let top = top.lines().next().unwrap_or_default().trim_end().to_string();
    assert!(top.ends_with("COPY") && !top.contains('+'), "a search that found all: {top:?}");
    s.app.stop();
}
