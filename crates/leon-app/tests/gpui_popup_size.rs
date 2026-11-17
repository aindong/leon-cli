//! A popup's program starts at the popup's size (.agent/plans/2026-11-17-popup-size.md). The app
//! tells the server its window's geometry (`client.geometry`), so a float or drawer made there
//! starts its PTY at exactly the cells the app then fits it to: the program's first `stty size` is
//! its size for good, and it gets no SIGWINCH. This is the parity check between the server's sum
//! (leon-core `Geometry`) and how the app lays a float or drawer out (`floats::place`,
//! `panes::text_rect`, `cells_in`).
mod layout_support;

use gpui::{px, size, TestAppContext};
use layout_support::*;
use serde_json::json;

/// Prints its size, then `W` for each SIGWINCH.
const SIZED: &str = "stty -echo; stty size; trap 'echo W' WINCH; while :; do sleep 0.05; done";

/// Make `label` with `session.toggle` (a float when `float`), wait until the app drew its size
/// line and every motion ended; returns (the first line, its PTY's size now, its SIGWINCHes), then
/// closes it.
fn made(
    s: &Setup,
    ws: &gpui::Entity<leon_app::view::Workspace>,
    cx: &mut gpui::VisualTestContext,
    label: &str,
    float: bool,
) -> (String, String, usize) {
    let sid = sid(ws, cx);
    let options = json!({ "command": ["sh", "-c", SIZED] });
    let p = json!({ "session_id": sid, "label": label, "create": true, "float": float, "options": options });
    s.app.call("session.toggle", p).unwrap();
    let st = s.app.ctx().state();
    let id = st.overlay_by_label(&sid, label).unwrap().expect("made");
    drop(st);
    let line = |cx: &mut gpui::VisualTestContext| {
        let text = ws.read_with(cx, |ws, _| ws.grid_drew(&id)).map(|d| d.text).unwrap_or_default();
        text.lines().find(|l| !l.trim().is_empty()).map(|l| l.trim().to_string())
    };
    assert!(shows(cx, |cx| line(cx).is_some()), "{label} never drew its size");
    to_rest(ws, cx);
    // the app's fit, if it differs, lands a moment later: give it the time a resize takes
    run_for(cx, std::time::Duration::from_millis(300));
    let (rows, cols) = term(&s.app, &id).size();
    let made = (line(cx).unwrap(), format!("{rows} {cols}"), winched(s, &id));
    close(s, &sid, &id);
    to_rest(ws, cx);
    made
}

#[gpui::test]
fn a_popup_and_a_drawer_start_at_the_size_the_app_gives_them(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    tab_running(&s, &ws, cx, &quiet("a-ready"), "a-ready");
    to_rest(&ws, cx);
    // window sizes, odd and fractional, and a bigger font: the server's sum must match the app's
    // layout (taffy rounds, a float has a border) in each
    let mut wrong = Vec::new();
    for (i, (w, h)) in [(1400., 900.), (1033., 777.), (1291.5, 853.5), (1120., 701.)].into_iter().enumerate() {
        cx.simulate_resize(size(px(w), px(h)));
        if i == 3 {
            act(&ws, cx, "client.font.increase");
        }
        to_rest(&ws, cx);
        for (label, float) in [(format!("pop{i}"), true), (format!("drawer{i}"), false)] {
            let (first, now, winches) = made(&s, &ws, cx, &label, float);
            if first != now || winches != 0 {
                wrong.push(format!("{label} at {w}x{h}: started {first}, fitted {now}, {winches} SIGWINCH"));
            }
        }
    }
    s.app.stop();
    assert!(wrong.is_empty(), "started at another size than the app gave it: {wrong:#?}");
}

#[gpui::test]
fn the_app_reports_its_geometry_only_when_it_changes(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    tab_running(&s, &ws, cx, &quiet("a-ready"), "a-ready");
    to_rest(&ws, cx);
    let sid = sid(&ws, cx);
    let (w, _) = window(&s.app, &ws, cx);
    let wid = w.id.clone();
    let stored = |s: &Setup| s.app.ctx().geometry(&wid);
    assert!(shows(cx, |_| stored(&s).is_some()), "the app reported its window's geometry");
    let real = stored(&s).unwrap();
    // another client's report (say, a second app) replaces it
    let fake = json!({ "session_id": sid, "window_id": wid, "area": [100.0, 100.0], "cell": [10.0, 20.0],
                       "float_frame": [0.0, 0.0], "drawer_frame": [0.0, 0.0] });
    s.app.call("client.geometry", fake).unwrap();
    let faked = stored(&s).unwrap();
    assert_ne!(faked, real);
    // frames and notifies with nothing changed send nothing: the fake stays
    for _ in 0..3 {
        ws.update(cx, |_, cx| cx.notify());
        cx.run_until_parked();
    }
    let kept = stored(&s) == Some(faked);
    // the window resized: the app reports again
    cx.simulate_resize(size(px(1200.), px(800.)));
    let resized = shows(cx, |_| stored(&s).is_some_and(|g| g != faked && g != real));
    let after_resize = stored(&s);
    // a bigger font: a bigger cell, the same area
    act(&ws, cx, "client.font.increase");
    let font =
        shows(cx, |_| stored(&s).is_some_and(|g| Some(g) != after_resize && g.area == after_resize.unwrap().area));
    s.app.stop();
    assert!(kept, "an unchanged window reported again");
    assert!(resized, "a resized window didn't report its new geometry");
    assert!(font, "a font change didn't report the new cell");
}
