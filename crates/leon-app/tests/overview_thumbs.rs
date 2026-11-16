//! Overview criteria 8 and 9 (.agent/plans/2026-11-17-overview-grid.md): a card draws its tab's
//! panes as laid out from the screens the server pushes, resizes nothing, watches what it shows
//! and only while open, and draws no pane faded.

mod m10b_support;

use std::sync::Arc;

use std::time::{Duration, Instant};

use gpui::{TestAppContext, VisualTestContext};
use m10b_support::*;

/// `shows`, with the test clock moving too: a thumbnail's refresh that `THUMB_EVERY` held back
/// is a timer on it, which real time alone never fires in a test window.
fn ticks(cx: &mut VisualTestContext, mut pred: impl FnMut(&mut VisualTestContext) -> bool) -> bool {
    let end = Instant::now() + Duration::from_secs(20);
    while !pred(cx) && Instant::now() < end {
        cx.executor().advance_clock(Duration::from_millis(50));
        cx.run_until_parked();
        std::thread::sleep(Duration::from_millis(20));
    }
    pred(cx)
}

#[gpui::test]
fn a_card_draws_each_pane_of_its_tab_and_resizes_none(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    // closing then drops the overview at once, not after its motion (overview_motion.rs)
    ws.update(cx, |ws, _| ws.set_reduce_motion(true));
    act(&ws, cx, "pane.split.right");
    let (w, _) = window(&s.app, &ws, cx);
    let ids = leaves(&w);
    assert_eq!(ids.len(), 2);
    assert!(shows(cx, |cx| ws.read_with(cx, |ws, _| ids.iter().all(|id| ws.screen_text(id).is_some()))));
    act(&ws, cx, "client.tab.new");
    let (other, _) = window(&s.app, &ws, cx);
    let sizes = |s: &Setup| ids.iter().map(|id| term(&s.app, id).size()).collect::<Vec<_>>();
    let before = sizes(&s);

    act(&ws, cx, "client.overview.toggle");
    for id in &ids {
        let pane = format!("card-pane-{}-{id}", w.id);
        assert!(ticks(cx, |cx| drawn(cx, &pane)), "{pane}: each pane of the split tab is drawn");
    }
    let watched = ws.read_with(cx, |ws, _| ws.watching());
    for id in ids.iter().chain(leaves(&other).iter()) {
        assert!(watched.contains(id), "{id} is watched while its card shows: {watched:?}");
    }
    cx.run_until_parked();
    assert_eq!(sizes(&s), before, "no pane was resized to its thumbnail");

    act(&ws, cx, "client.overview.toggle");
    let watched = ws.read_with(cx, |ws, _| ws.watching());
    assert_eq!(watched, leaves(&other), "once closed, only the panes on screen are watched");
    s.app.stop();
}

#[gpui::test]
fn thumbnails_are_never_faded(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    act(&ws, cx, "pane.split.right");
    let (w, _) = window(&s.app, &ws, cx);
    let ids = leaves(&w);
    let focused = ws.read_with(cx, |ws, _| ws.target_block()).unwrap();
    let other = ids.iter().find(|id| **id != focused).unwrap().clone();
    let faded = shows(cx, |cx| ws.read_with(cx, |ws, _| ws.pane_theme(&other) != ws.theme()));
    assert!(faded, "the pane without the focus is faded on screen");

    act(&ws, cx, "client.overview.toggle");
    let ready = |cx: &mut gpui::VisualTestContext| {
        ws.read_with(cx, |ws, _| ids.iter().all(|id| ws.thumb_pane(&w.id, id).is_some()))
    };
    assert!(ticks(cx, ready), "the card has both panes' screens");
    for id in &ids {
        let (_, theme) = ws.read_with(cx, |ws, _| ws.thumb_pane(&w.id, id).unwrap());
        assert_eq!(theme, ws.read_with(cx, |ws, _| ws.theme()), "{id} is drawn in the window's theme");
        // the screen as pushed, once the thumbnail caught up with it (a shell still drawing its
        // prompt pushes a new one), never a faded copy
        let same = |cx: &mut VisualTestContext| {
            ws.read_with(cx, |ws, _| {
                let thumb = ws.thumb_pane(&w.id, id).map(|(f, _)| f);
                thumb.zip(ws.screen(id)).is_some_and(|(f, pushed)| Arc::ptr_eq(&f, &pushed))
            })
        };
        assert!(ticks(cx, same), "{id} is drawn from the screen as pushed, not a faded copy");
    }
    s.app.stop();
}
