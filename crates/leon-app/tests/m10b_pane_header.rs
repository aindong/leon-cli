//! M10b criterion 6: pane headers name the pane, carry its status and caption, offer split/zoom/
//! close on hover, and can be hidden (the terminal takes the rows back).

mod m10b_support;

use gpui::{Modifiers, TestAppContext};
use m10b_support::*;

#[gpui::test]
fn each_pane_has_a_header_above_its_terminal(cx: &mut TestAppContext) {
    // a shell that sets no title of its own (a login zsh's prompt may name the window user@host),
    // so the header names the program
    std::env::set_var("SHELL", "/bin/sh");
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    cx.simulate_keystrokes("cmd-d");
    let (w, _) = window(&s.app, &ws, cx);
    for id in leaves(&w) {
        let header = at(cx, &format!("header-{id}")).expect("a header per pane");
        assert_eq!(f32::from(header.size.height), 24.);
        let term = at(cx, &format!("terminal-{id}")).unwrap();
        assert!(header.bottom() <= term.top(), "above its terminal");
        let title = ws.read_with(cx, |ws, _| ws.pane_title(&id)).expect("a title");
        assert!(title.contains("zsh") || title.contains("sh"), "names the program: {title}");
    }
    s.app.stop();
}

#[gpui::test]
fn the_header_controls_split_zoom_and_close(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let id = ws.read_with(cx, |ws, _| ws.target_block()).unwrap();
    let header = at(cx, &format!("header-{id}")).unwrap();
    cx.simulate_mouse_move(center(header), None, Modifiers::none()); // the controls show on hover
    let split = at(cx, &format!("header-split-right-{id}")).expect("a split-right control");
    assert!(header.contains(&center(split)));
    cx.simulate_click(center(split), Modifiers::none());
    assert!(shows(cx, |cx| leaves(&window(&s.app, &ws, cx).0).len() == 2), "the control split the pane");
    let new = ws.read_with(cx, |ws, _| ws.target_block()).unwrap();
    let p = center(at(cx, &format!("header-{new}")).unwrap());
    cx.simulate_mouse_move(p, None, Modifiers::none());
    let p = center(at(cx, &format!("header-zoom-{new}")).unwrap());
    cx.simulate_click(p, Modifiers::none());
    assert_eq!(window(&s.app, &ws, cx).0.zoomed.as_deref(), Some(new.as_str()), "zoom zooms it");
    let p = center(at(cx, &format!("header-{new}")).unwrap());
    cx.simulate_mouse_move(p, None, Modifiers::none());
    let p = center(at(cx, &format!("header-close-{new}")).unwrap());
    cx.simulate_click(p, Modifiers::none());
    assert!(shows(cx, |cx| leaves(&window(&s.app, &ws, cx).0) == vec![id.clone()]), "close closes it");
    s.app.stop();
}

#[gpui::test]
fn hiding_headers_gives_the_terminal_the_rows(cx: &mut TestAppContext) {
    let s = setup();
    let (ws, cx) = open(cx, &s.app);
    let id = ws.read_with(cx, |ws, _| ws.target_block()).unwrap();
    let t = term(&s.app, &id);
    let with = t.size().0;
    act(&ws, cx, "client.pane_header.reveal");
    assert!(!ws.read_with(cx, |ws, _| ws.settings().pane_headers));
    assert!(!drawn(cx, &format!("header-{id}")), "no header");
    assert!(eventually(|| t.size().0 > with), "the terminal took the header's rows: {with} -> {}", t.size().0);
    act(&ws, cx, "client.pane_header.reveal");
    assert!(drawn(cx, &format!("header-{id}")), "shown again");
    let rows = ws.read_with(cx, |ws, _| ws.palette_rows_for("pane header"));
    assert!(rows.iter().any(|(t, _)| t == "Show or Hide Pane Headers"), "in the palette: {rows:?}");
    s.app.stop();
}
