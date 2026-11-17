//! Motion PR 2: a zoom grows the pane to fill the tab and an unzoom shrinks it back. Its sibling
//! stays drawn beneath until the zoom ends and from the unzoom's start, and the moving pane is
//! drawn last, so nothing paints over it.
mod layout_support;

use gpui::TestAppContext;
use layout_support::*;
use leon_app::layout_motion::LAYOUT;

#[gpui::test]
fn zoom_grows_the_pane_and_unzoom_shrinks_it_back(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    ws.update(cx, |ws, _| ws.set_reduce_motion(false));
    let a = tab_running(&s, &ws, cx, &quiet("a-ready"), "a-ready");
    let b = split_running(&s, &ws, cx, &a, &quiet("b-ready"), "b-ready");
    to_rest(&ws, cx);
    let (a0, b0) = (tile(cx, &a).unwrap(), tile(cx, &b).unwrap());
    let sid = sid(&ws, cx);
    let last = |cx: &mut gpui::VisualTestContext| ws.read_with(cx, |ws, _| ws.tile_order().last().cloned());

    zoom(&s, &sid, &a, true);
    assert!(moving(&ws, cx), "the zoom moves");
    frame(&ws, cx, LAYOUT / 2);
    let (a1, b1, top1) = (tile(cx, &a).unwrap(), tile(cx, &b), last(cx));
    to_rest(&ws, cx);
    let (a2, b2) = (tile(cx, &a).unwrap(), tile(cx, &b));

    zoom(&s, &sid, &a, false);
    assert!(moving(&ws, cx), "the unzoom moves");
    let (b3, top3) = (tile(cx, &b), last(cx));
    frame(&ws, cx, LAYOUT / 2);
    let a4 = tile(cx, &a).unwrap();
    to_rest(&ws, cx);
    let (a5, b5) = (tile(cx, &a).unwrap(), tile(cx, &b).unwrap());
    s.app.stop();

    assert!(a1.2 > a0.2 && a1.2 < a2.2, "half way the zoomed pane is between: {a0:?} {a1:?} {a2:?}");
    assert_eq!(b1, Some(b0), "its sibling stays drawn where it was while the zoom grows");
    assert_eq!(top1.as_deref(), Some(a.as_str()), "the zoomed pane is drawn last");
    assert!(a2.2 > b0.0 - a0.0 + b0.2 - 1., "zoomed, it fills the tile area: {a2:?}");
    assert_eq!(b2, None, "and its sibling isn't drawn");
    assert_eq!(b3, Some(b0), "unzooming, its sibling is drawn at its place from the first frame");
    assert_eq!(top3.as_deref(), Some(a.as_str()), "and the shrinking pane is drawn last");
    assert!(a4.2 < a2.2 && a4.2 > a0.2, "half way it is between: {a4:?}");
    assert_eq!((a5, b5), (a0, b0), "and both end where they were");
}

/// Review (a GPUI panic, `line_layout.rs` reuse_layouts): a pane closed while zoomed is both what
/// leaves and the zoom's pane; it is drawn once a frame, never twice.
#[gpui::test]
fn a_pane_closed_while_zoomed_is_drawn_once(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    ws.update(cx, |ws, _| ws.set_reduce_motion(false));
    let a = tab_running(&s, &ws, cx, &quiet("a-ready"), "a-ready");
    let b = split_running(&s, &ws, cx, &a, &quiet("b-ready"), "b-ready");
    to_rest(&ws, cx);
    let sid = sid(&ws, cx);
    zoom(&s, &sid, &b, true);
    assert!(moving(&ws, cx), "the zoom moves");
    to_rest(&ws, cx);
    close(&s, &sid, &b);
    assert!(moving(&ws, cx), "the close moves");
    let mut orders = vec![ws.read_with(cx, |ws, _| ws.tile_order())];
    while ws.read_with(cx, |ws, _| ws.frame_asked()) {
        frame(&ws, cx, FRAME);
        orders.push(ws.read_with(cx, |ws, _| ws.tile_order()));
    }
    let end = tile(cx, &a);
    s.app.stop();
    for o in &orders {
        let mut sorted = o.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(o.len(), sorted.len(), "each pane drawn once a frame: {o:?}");
    }
    assert!(end.is_some(), "the pane left fills the window");
}

/// Flakes round 7 (m6b_panes `zoom_never_hides_where_input_goes`): keys typed while a zoom from
/// the CLI is still moving go to the zoomed pane, the one input follows, not the focused sibling.
#[gpui::test]
fn keys_typed_while_a_zoom_moves_reach_the_zoomed_pane(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    ws.update(cx, |ws, _| ws.set_reduce_motion(false));
    let reads = "stty -echo; echo a-ready; read l; echo \"got:$l\"; exec sleep 300";
    let a = tab_running(&s, &ws, cx, reads, "a-ready");
    let b = split_running(&s, &ws, cx, &a, &quiet("b-ready"), "b-ready");
    to_rest(&ws, cx);
    let sid = sid(&ws, cx);

    zoom(&s, &sid, &a, true);
    assert!(moving(&ws, cx), "the zoom moves");
    frame(&ws, cx, LAYOUT / 2);
    assert!(ws.read_with(cx, |ws, _| ws.animating()), "still moving");
    cx.simulate_input("hi");
    cx.simulate_keystrokes("enter");
    let got = eventually(|| term(&s.app, &a).text(true).lines().any(|l| l.trim() == "got:hi"));
    let other = term(&s.app, &b).text(true);
    s.app.stop();
    assert!(got, "the zoomed pane's program read the line");
    assert!(!other.contains("hi"), "its sibling got nothing: {other}");
}
