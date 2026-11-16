//! A screen pushed while its grid syncs (between `sync_grid`'s frame read and its update-count
//! read) must still be drawn: the poll has to see that count as not yet drawn.
mod grid_support;

use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui::TestAppContext;
use grid_support::*;

/// A screen full of one-cell colour spans (a costly compose), then one overwrite per line read.
const SCRIPT: &str = r#"stty -echo; exec perl -e '$|=1; for $r (1..60) { print join("", map { "\e[3" . ($_ % 8) . "mx" } 1..150), "\e[0m\n" } print "ready\n"; while (<STDIN>) { chomp; print "\e[1;1Hmark-$_ " }'"#;

#[gpui::test]
fn a_screen_pushed_while_its_grid_syncs_is_drawn(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let id = tab_running(&s, &ws, cx, SCRIPT, "ready");
    let t = term(&s.app, &id);
    assert!(shows(cx, |cx| stays(cx, &t, t.size())), "the pane keeps its size");
    run_for(cx, Duration::from_millis(300));
    // the race landed by trial 18 when it was there; 150 trials miss it well under 1% of the time
    for k in 0..150u32 {
        let marker = format!("mark-{k:04}");
        // held, so the reader's push makes a new Arc: a pointer change says a push landed
        let before = ws.read_with(cx, |ws, _| ws.screen(&id)).expect("a screen");
        t.write(format!("{k:04}\n").as_bytes()).unwrap();
        let end = Instant::now() + Duration::from_secs(10);
        loop {
            // what every workspace notify runs: prepare, then sync_grids
            ws.update(cx, |_, cx| cx.notify());
            let now = ws.read_with(cx, |ws, _| ws.screen(&id)).expect("a screen");
            if !Arc::ptr_eq(&now, &before) {
                break;
            }
            assert!(Instant::now() < end, "no screen push came for {marker}");
        }
        drop(before);
        // quiet now: only the poll, woken by that push, can bring the grid up to date
        let drew =
            shows(cx, |cx| ws.read_with(cx, |ws, _| ws.grid_drew(&id)).is_some_and(|d| d.text.contains(&marker)));
        if !drew {
            let model = ws.read_with(cx, |ws, _| ws.screen_text(&id)).unwrap_or_default().contains(&marker);
            s.app.stop();
            assert!(model, "the model never got {marker}");
            panic!("trial {k}: the model has {marker}, but the grid still draws the screen before it");
        }
    }
    s.app.stop();
}
