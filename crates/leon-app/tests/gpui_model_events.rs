//! GPUI phase 5 (ADR R1, R4): the workspace hears of the model through each machine's model
//! entity and notifies only the views whose drawing changed. A status a pane reports (a refetch
//! the root draws nothing of) renders its header and leaves the root un-notified; a pushed screen
//! renders its grid alone; a layout change from the CLI notifies the root.
mod m20b_support;

use gpui::{Entity, TestAppContext, VisualTestContext};
use leon_app::view::Workspace;
use m20b_support::*;
use serde_json::json;

const WORKING: &str = r"printf '\033]7501;state=working\007'; echo w-sent";

/// How many times the root was told it draws something new (GPUI renders it on every frame it
/// draws, a header's or a grid's too, so its render count is the frames').
fn root(ws: &Entity<Workspace>, cx: &mut VisualTestContext) -> u64 {
    cx.run_until_parked();
    ws.read_with(cx, |ws, _| ws.root_redraws())
}

fn header(ws: &Entity<Workspace>, cx: &mut VisualTestContext, id: &str) -> u64 {
    ws.read_with(cx, |ws, _| ws.section_renders().get(&format!("header-{id}")).copied().unwrap_or(0))
}

fn grid(ws: &Entity<Workspace>, cx: &mut VisualTestContext, id: &str) -> u64 {
    ws.read_with(cx, |ws, _| ws.grid_renders().get(id).copied().unwrap_or(0))
}

#[gpui::test]
fn each_model_change_renders_only_what_draws_it(cx: &mut TestAppContext) {
    let s = plain_setup();
    let (ws, cx) = open(cx, &s.app);
    act(&ws, cx, "pane.split.right");
    assert!(shows(cx, |cx| leaves(&window(&s.app, &ws, cx).0).len() == 2), "split");
    let (w, _) = window(&s.app, &ws, cx);
    let [a, b] = <[String; 2]>::try_from(leaves(&w)).unwrap();
    assert!(shows(cx, |cx| drawn(cx, &format!("pane-{b}"))), "the split is drawn");
    settle(&term(&s.app, &a));
    settle(&term(&s.app, &b));

    // a status from the unfocused pane: its header and the sidebar draw it, the root doesn't
    let (r0, h0) = (root(&ws, cx), header(&ws, cx, &a));
    run_until(&term(&s.app, &a), WORKING, "w-sent");
    let working = |cx: &mut VisualTestContext| {
        ws.read_with(cx, |ws, _| ws.header_caption(&a)).as_deref() == Some("working") && header(&ws, cx, &a) > h0
    };
    assert!(shows(cx, working), "a's header drew its working state");
    settle(&term(&s.app, &a));
    assert_eq!(root(&ws, cx), r0, "a status report notified the root");

    // a pushed screen: its grid, not the root
    let (r1, g1) = (root(&ws, cx), grid(&ws, cx, &b));
    run_until(&term(&s.app, &b), "echo pushed", "pushed");
    assert!(shows(cx, |cx| grid(&ws, cx, &b) > g1), "b's grid drew its new screen");
    settle(&term(&s.app, &b));
    assert_eq!(root(&ws, cx), r1, "a pushed screen notified the root");

    // a split from the CLI changes the layout the root draws
    let sid = sid(&ws, cx);
    let p = json!({ "session_id": sid, "anchor_id": b, "direction": "vertical", "side": "after",
                        "layout": { "block": {} } });
    let r2 = root(&ws, cx);
    s.app.call("session.new_split", p).unwrap();
    // read from the window's model copy, never `drawn` (its refresh renders every view)
    let three = |cx: &mut VisualTestContext| {
        let leaves = ws.read_with(cx, |ws, _| ws.active_window()).map(|w| leaves(&w).len());
        leaves == Some(3) && root(&ws, cx) > r2
    };
    let rendered = shows(cx, three);
    s.app.stop();
    assert!(rendered, "the CLI's split reached the window's model and notified the root");
}
