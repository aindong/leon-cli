//! Audit A3 (the 0.1.0 app audit): copying from a host's pane copies the text the app drew,
//! whatever the host's `selection` reply says, and a link whose `link_at` uri isn't the text drawn
//! there asks before it opens. Audit A4: once the host goes, its panes' history rows go too.

// macOS only: drives the GPUI app through support/app.rs (the `app` feature).
#![cfg(all(feature = "app", target_os = "macos"))]

#[path = "support/app.rs"]
mod app;
mod support;

use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use app::*;
use gpui::{Modifiers, MouseButton, MouseDownEvent, MouseUpEvent, TestAppContext, VisualTestContext};

fn row_of(ws: &gpui::Entity<leon_app::view::Workspace>, cx: &mut VisualTestContext, id: &str, text: &str) -> usize {
    let shown = ws.read_with(cx, |ws, _| ws.view_text(id)).unwrap_or_default();
    shown.lines().position(|l| l.trim_end() == text).unwrap_or_else(|| panic!("no row shows {text:?}:\n{shown}"))
}

fn click(cx: &mut VisualTestContext, at: gpui::Point<gpui::Pixels>, modifiers: Modifiers) {
    let left = MouseButton::Left;
    cx.simulate_event(MouseDownEvent { button: left, position: at, modifiers, click_count: 1, first_mouse: false });
    cx.simulate_event(MouseUpEvent { button: left, position: at, modifiers, click_count: 1 });
    cx.run_until_parked();
}

/// Drag over the row that shows `text`, again until the selection holds (the pane's first resize
/// renumbers its rows, which drops a selection).
fn drag_over(ws: &gpui::Entity<leon_app::view::Workspace>, cx: &mut VisualTestContext, b: &str, text: &str) {
    let none = Modifiers::none();
    let selected = shows(cx, |cx| {
        let row = row_of(ws, cx, b, text);
        let at = |cx: &mut VisualTestContext, col| ws.read_with(cx, |ws, _| ws.cell_position(b, row, col)).unwrap();
        let (from, to) = (at(cx, 0), at(cx, text.chars().count() - 1));
        let left = MouseButton::Left;
        cx.simulate_event(MouseDownEvent {
            button: left,
            position: from,
            modifiers: none,
            click_count: 1,
            first_mouse: false,
        });
        cx.simulate_mouse_move(to, left, none);
        cx.simulate_event(MouseUpEvent { button: left, position: to, modifiers: none, click_count: 1 });
        cx.run_until_parked();
        ws.read_with(cx, |ws, _| ws.highlighted_text(b)).as_deref() == Some(text)
    });
    assert!(selected, "the drag selected {text:?}: {:?}", ws.read_with(cx, |ws, _| ws.highlighted_text(b)));
}

/// The clipboard once it no longer holds `was` (10 s at most).
fn clipboard_after(cx: &mut VisualTestContext, was: &str) -> Option<String> {
    let now = |cx: &mut VisualTestContext| cx.read_from_clipboard().and_then(|c| c.text());
    let end = Instant::now() + Duration::from_secs(10);
    while now(cx).as_deref() == Some(was) && Instant::now() < end {
        cx.run_until_parked();
        std::thread::sleep(Duration::from_millis(20));
    }
    now(cx)
}

#[gpui::test]
fn a_hosts_replies_dont_decide_what_is_copied_or_opened(cx: &mut TestAppContext) {
    let h = hosted();
    std::env::set_var("LEON_LINK_HOLD_SECS", "3"); // the link's hold, 60 s by default
    let script = "stty -echo; seq 1 200; printf 'copy me please\\ngo https://good.example/page now\\nready-remote\\n'; sleep 300";
    let (sid, b) = session_running(&h.r.remote, "api", script);
    let (ws, cx) = open(cx, &h, vec![host("hostile", "ssh://hostile")], None);
    wait_connected(&ws, cx, "hostile");
    assert!(shows(cx, |cx| ws.read_with(cx, |ws, _| ws.host_rows()[1].sessions.iter().any(|s| s.id == sid))));
    select(&ws, cx, &sid);
    let ok = shows(cx, |cx| ws.read_with(cx, |ws, _| ws.view_text(&b).is_some_and(|t| t.contains("ready-remote"))));
    assert!(ok);
    std::fs::write(h.r.fake.path().join("lie"), "").unwrap();
    let at = |cx: &mut VisualTestContext, row, col| ws.read_with(cx, |ws, _| ws.cell_position(&b, row, col)).unwrap();

    // a selection's text: the cells drawn, not the reply's (dragged again while the pane's first
    // resize renumbers its rows, which drops a selection)
    let none = Modifiers::none();
    let selected = shows(cx, |cx| {
        let row = row_of(&ws, cx, &b, "copy me please");
        let (from, to) = (at(cx, row, 0), at(cx, row, 13));
        let left = MouseButton::Left;
        cx.simulate_event(MouseDownEvent {
            button: left,
            position: from,
            modifiers: none,
            click_count: 1,
            first_mouse: false,
        });
        cx.simulate_mouse_move(to, left, none);
        cx.simulate_event(MouseUpEvent { button: left, position: to, modifiers: none, click_count: 1 });
        for _ in 0..10 {
            cx.run_until_parked();
            std::thread::sleep(Duration::from_millis(20));
        }
        ws.read_with(cx, |ws, _| ws.highlighted_text(&b)).as_deref() == Some("copy me please")
    });
    assert!(selected, "the drag selected the row: {:?}", ws.read_with(cx, |ws, _| ws.highlighted_text(&b)));
    act(&ws, cx, "pane.copy", None);
    let end = Instant::now() + Duration::from_secs(10);
    while cx.read_from_clipboard().is_none() && Instant::now() < end {
        cx.run_until_parked();
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(cx.read_from_clipboard().and_then(|c| c.text()).as_deref(), Some("copy me please"));

    // a link whose reply names another uri asks first
    let opened = Rc::new(RefCell::new(Vec::<String>::new()));
    let o = opened.clone();
    ws.update(cx, |ws, _| ws.set_opener(Box::new(move |uri: &str| o.borrow_mut().push(uri.to_string()))));
    let link_row = row_of(&ws, cx, &b, "go https://good.example/page now");
    let link = at(cx, link_row, 8);
    let cmd = Modifiers { platform: true, ..Modifiers::none() };
    cx.simulate_mouse_move(link, None::<MouseButton>, cmd);
    assert!(shows(cx, |cx| ws.read_with(cx, |ws, _| ws.hovered_link()).is_some()), "the link is under the mouse");
    click(cx, link, cmd);
    assert!(opened.borrow().is_empty(), "opened without asking: {:?}", opened.borrow());
    assert!(cx.has_pending_prompt(), "a uri that isn't the text drawn asks first");
    cx.simulate_prompt_answer("Cancel");
    cx.run_until_parked();
    assert!(opened.borrow().is_empty());
    std::fs::remove_file(h.r.fake.path().join("lie")).unwrap();

    // a selection reply that widens the range can't make a copy take more than was dragged over
    for widen in [r#""start":{"col":0,"line":0}"#, r#""end":{"col":0,"line":3000000}"#] {
        std::fs::write(h.r.fake.path().join("widen"), widen).unwrap();
        cx.write_to_clipboard(gpui::ClipboardItem::new_string("before".into()));
        drag_over(&ws, cx, &b, "copy me please");
        act(&ws, cx, "pane.copy", None);
        assert_eq!(clipboard_after(cx, "before").as_deref(), Some("copy me please"), "with {widen}");
    }
    std::fs::remove_file(h.r.fake.path().join("widen")).unwrap();

    // copy mode's keys typed back to back: the yank copies what `v` and the motions selected
    cx.write_to_clipboard(gpui::ClipboardItem::new_string("before".into()));
    act(&ws, cx, "client.copy_mode", None);
    let up = row_of(&ws, cx, &b, "ready-remote") + 1 - row_of(&ws, cx, &b, "copy me please");
    let keys = format!("{} 0 v {} y", vec!["k"; up].join(" "), vec!["l"; 13].join(" "));
    cx.simulate_keystrokes(&keys);
    assert_eq!(clipboard_after(cx, "before").as_deref(), Some("copy me please"), "copy mode's yank");

    // the host goes: the rows its pane held go with it
    let held = shows(cx, |cx| {
        act(&ws, cx, "client.scroll.page_up", None);
        ws.read_with(cx, |ws, _| ws.cached_rows(&b)).is_some_and(|n| n > 0)
    });
    assert!(held, "history rows fetched: {:?}", ws.read_with(cx, |ws, _| ws.cached_rows(&b)));
    std::fs::write(h.r.fake.path().join("down"), "").unwrap();
    assert!(h.r.kill_transports() >= 3);
    // the link keeps trying for its hold (3 s here), then the host is gone
    let end = Instant::now() + Duration::from_secs(30);
    while ws.read_with(cx, |ws, _| ws.host_remote("hostile")).is_some() && Instant::now() < end {
        cx.run_until_parked();
        std::thread::sleep(Duration::from_millis(100));
    }
    assert!(ws.read_with(cx, |ws, _| ws.host_remote("hostile")).is_none(), "the link gave up");
    let freed = shows(cx, |cx| ws.read_with(cx, |ws, _| ws.cached_rows(&b)).is_none());
    assert!(freed, "{:?} history rows kept for a host that went", ws.read_with(cx, |ws, _| ws.cached_rows(&b)));
    h.local.stop();
}
