//! The app fades the panes that don't have the focus: the focused pane is at full strength and
//! the others faded, one pane alone is not faded, a focused float or drawer fades the tiles
//! behind it, and a window that isn't the key window fades nothing more.

mod m20b_support;

use gpui::{Entity, TestAppContext, VisualTestContext};
use leon_app::view::Workspace;
use m20b_support::*;
use serde_json::json;

/// The default strength.
const S: f32 = leon_lua::settings::FADE as f32;
/// The app's poll timer: a wait moves it on and never asks for a redraw itself.
const POLL: std::time::Duration = std::time::Duration::from_millis(8);

/// Wait until no fade is moving and `want` holds. Time alone moves a fade, so each look draws again.
fn rests(ws: &Entity<Workspace>, cx: &mut VisualTestContext, want: impl Fn(&Workspace) -> bool) -> bool {
    shows(cx, |cx| {
        cx.executor().advance_clock(POLL);
        cx.run_until_parked();
        ws.read_with(cx, |ws, _| !ws.fading() && want(ws))
    })
}

fn target(ws: &Entity<Workspace>, cx: &mut VisualTestContext) -> String {
    ws.read_with(cx, |ws, _| ws.target_block()).expect("a pane has the focus")
}

/// Split the focused pane to the right; returns (the pane that was focused, the new one, focused now).
fn split(app: &Setup, ws: &Entity<Workspace>, cx: &mut VisualTestContext) -> (String, String) {
    let first = target(ws, cx);
    let before = leaves(&window(&app.app, ws, cx).0).len();
    act(ws, cx, "pane.split.right");
    assert!(shows(cx, |cx| leaves(&window(&app.app, ws, cx).0).len() == before + 1), "no split");
    assert!(shows(cx, |cx| target(ws, cx) != first), "the new pane takes the focus");
    (first, target(ws, cx))
}

fn fades(ws: &Entity<Workspace>, cx: &mut VisualTestContext, ids: &[&String]) -> Vec<f32> {
    ws.read_with(cx, |ws, _| ids.iter().map(|id| ws.pane_fade(id)).collect())
}

#[gpui::test]
fn the_focused_pane_is_full_strength_and_the_others_faded(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let only = target(&ws, cx);
    let alone = rests(&ws, cx, |ws| ws.pane_fade(&only) == 0.);
    let (first, second) = split(&s, &ws, cx);
    let two = rests(&ws, cx, |ws| ws.pane_fade(&second) == 0. && ws.pane_fade(&first) == S);
    let after_split = fades(&ws, cx, &[&first, &second]);
    act(&ws, cx, "pane.focus.left");
    let moved = rests(&ws, cx, |ws| ws.pane_fade(&first) == 0. && ws.pane_fade(&second) == S);
    let after_focus = fades(&ws, cx, &[&first, &second]);
    act(&ws, cx, "pane.zoom");
    assert!(shows(cx, |cx| window(&s.app, &ws, cx).0.zoomed.is_some()), "no zoom");
    let zoomed = rests(&ws, cx, |ws| ws.pane_fade(&first) == 0.);
    s.app.stop();
    assert!(alone, "one pane alone is not faded");
    assert!(two, "the new pane has the focus and the first is faded to {S}: {after_split:?}");
    assert!(moved, "the fade follows the focus: {after_focus:?}");
    assert!(zoomed, "a zoomed pane is alone on screen: not faded");
}

#[gpui::test]
fn a_focused_float_or_drawer_fades_the_tiles_behind_it(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let (first, second) = split(&s, &ws, cx);
    let sid = sid(&ws, cx);
    let float = new_float(&s.app, &sid, [0.3, 0.2, 0.4, 0.4]);
    let floating = rests(&ws, cx, |ws| {
        ws.target_block().as_ref() == Some(&float)
            && ws.pane_fade(&float) == 0.
            && ws.pane_fade(&first) == S
            && ws.pane_fade(&second) == S
    });
    let with_float = fades(&ws, cx, &[&first, &second, &float]);

    s.app.call("session.toggle", json!({ "session_id": sid, "label": "scratch", "create": true })).unwrap();
    let drawer = view_window(&s.app, &sid)["docked"][0]["block_id"].as_str().unwrap().to_string();
    s.app.call("session.focus_block", json!({ "session_id": sid, "block_id": drawer })).unwrap();
    let docked = rests(&ws, cx, |ws| {
        ws.target_block().as_ref() == Some(&drawer)
            && ws.pane_fade(&drawer) == 0.
            && [&first, &second, &float].iter().all(|id| ws.pane_fade(id) == S)
    });
    let with_drawer = fades(&ws, cx, &[&first, &second, &float, &drawer]);

    // back to a tile: the float and the drawer are the ones faded
    s.app.call("session.focus_block", json!({ "session_id": sid, "block_id": first })).unwrap();
    let tiled = rests(&ws, cx, |ws| {
        ws.pane_fade(&first) == 0. && [&second, &float, &drawer].iter().all(|id| ws.pane_fade(id) == S)
    });
    s.app.stop();
    assert!(floating, "the float has the focus, the tiles are faded: {with_float:?}");
    assert!(docked, "the drawer has the focus, everything else is faded: {with_drawer:?}");
    assert!(tiled, "a focused tile fades the float and the drawer");
}

#[gpui::test]
fn a_window_that_is_not_key_fades_nothing_more(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let (first, second) = split(&s, &ws, cx);
    // a test window is not the key window until it is activated
    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();
    let was_key = cx.update(|window, _| window.is_window_active());
    let key = rests(&ws, cx, |ws| ws.pane_fade(&second) == 0. && ws.pane_fade(&first) == S);
    cx.deactivate_window();
    cx.run_until_parked();
    let inactive = cx.update(|window, _| !window.is_window_active());
    let same = rests(&ws, cx, |ws| ws.pane_fade(&second) == 0. && ws.pane_fade(&first) == S);
    let (focused_theme, window_theme) = ws.read_with(cx, |ws, _| (ws.pane_theme(&second), ws.theme()));
    s.app.stop();
    assert!(was_key, "the test's window became the key window");
    assert!(key, "at rest in the key window");
    assert!(inactive, "the test's window stopped being the key window");
    assert!(same, "one state, not two stacked: the same fades in a window that isn't key");
    assert_eq!(focused_theme, window_theme, "the focused pane stays at full strength");
}
