//! Popups (.agent/plans/2026-11-17-popup-toggle.md, criterion 10): an overlay a toggle shows
//! gives the focus back to the block that had it when it hides or closes.

use leon_core::layout::{Direction, Side};
use leon_core::overlays::Bounds;
use leon_core::terminal::{Spec, Terminal};
use leon_core::{Block, Place, State};
use std::sync::Arc;

fn add(st: &mut State, s: &str, w: &str, id: &str, label: Option<&str>, anchor: Option<&str>) {
    let spec = Spec { command: vec!["sleep".into(), "300".into()], cwd: None, env: vec![] };
    let terminal = Arc::new(Terminal::spawn(spec, |_| {}).unwrap());
    let b = Block {
        id: id.into(),
        label: label.map(Into::into),
        window_id: w.into(),
        keep_open: false,
        exit_code: None,
        terminal,
    };
    let place = anchor.map(|a| Place {
        anchor: a.into(),
        direction: Direction::Horizontal,
        side: Side::After,
        ratio: 0.5,
        focus: true,
    });
    st.add_block(s, b, place.as_ref()).unwrap();
}

fn focus(st: &State, s: &str) -> Option<String> {
    st.session(s).unwrap().windows[0].focus.clone()
}

#[test]
fn hiding_and_closing_a_toggled_overlay_give_the_focus_back() {
    let mut st = State::default();
    let (s, w) = st.create_session(None, None).unwrap();
    add(&mut st, &s, &w, "block:a", None, None);
    add(&mut st, &s, &w, "block:b", None, Some("block:a"));
    add(&mut st, &s, &w, "block:pop", Some("pop"), Some("block:b"));
    st.float_block(&s, "block:pop", Bounds::default()).unwrap();
    st.toggle_overlay(&s, "pop").unwrap(); // hidden
    st.focus_block(&s, "block:b").unwrap();

    // b is not the first tile, so only a remembered focus lands there
    st.toggle_overlay(&s, "pop").unwrap();
    assert_eq!(focus(&st, &s).as_deref(), Some("block:pop"), "showing focuses the popup");
    st.toggle_overlay(&s, "pop").unwrap();
    assert_eq!(focus(&st, &s).as_deref(), Some("block:b"), "hiding gives the focus back");

    st.toggle_overlay(&s, "pop").unwrap();
    st.close_block(&s, "block:pop").unwrap();
    assert_eq!(focus(&st, &s).as_deref(), Some("block:b"), "closing gives the focus back");
}

#[test]
fn closing_a_focused_float_never_leaves_the_focus_on_it() {
    let mut st = State::default();
    let (s, w) = st.create_session(None, None).unwrap();
    add(&mut st, &s, &w, "block:a", None, None);
    add(&mut st, &s, &w, "block:f", None, Some("block:a"));
    st.float_block(&s, "block:f", Bounds::default()).unwrap();
    assert_eq!(focus(&st, &s).as_deref(), Some("block:f"));
    st.close_block(&s, "block:f").unwrap();
    assert_eq!(focus(&st, &s).as_deref(), Some("block:a"));
}

#[test]
fn a_return_target_that_closed_falls_back_to_the_usual_pick() {
    let mut st = State::default();
    let (s, w) = st.create_session(None, None).unwrap();
    add(&mut st, &s, &w, "block:a", None, None);
    add(&mut st, &s, &w, "block:b", None, Some("block:a"));
    add(&mut st, &s, &w, "block:pop", Some("pop"), Some("block:b"));
    st.float_block(&s, "block:pop", Bounds::default()).unwrap();
    st.toggle_overlay(&s, "pop").unwrap();
    st.focus_block(&s, "block:b").unwrap();
    st.toggle_overlay(&s, "pop").unwrap();
    st.close_block(&s, "block:b").unwrap();
    st.toggle_overlay(&s, "pop").unwrap();
    assert_eq!(focus(&st, &s).as_deref(), Some("block:a"));
}

#[test]
fn a_popup_that_lost_the_focus_leaves_it_where_it_is() {
    let mut st = State::default();
    let (s, w) = st.create_session(None, None).unwrap();
    add(&mut st, &s, &w, "block:a", None, None);
    add(&mut st, &s, &w, "block:b", None, Some("block:a"));
    add(&mut st, &s, &w, "block:c", None, Some("block:b"));
    add(&mut st, &s, &w, "block:pop", Some("pop"), Some("block:c"));
    st.float_block(&s, "block:pop", Bounds::default()).unwrap();
    st.toggle_overlay(&s, "pop").unwrap();
    st.focus_block(&s, "block:b").unwrap();
    st.toggle_overlay(&s, "pop").unwrap(); // shown from b
    st.focus_block(&s, "block:c").unwrap();

    st.toggle_overlay(&s, "pop").unwrap();
    assert_eq!(focus(&st, &s).as_deref(), Some("block:c"), "hiding an unfocused popup");
    // its program exits while hidden
    st.block_exited("block:pop", 0);
    assert!(st.block("block:pop").is_err(), "closed on exit");
    assert_eq!(focus(&st, &s).as_deref(), Some("block:c"), "closing an unfocused popup");
}
