//! A popup toggled from another tab gives the focus back to the pane the key was pressed in.

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

#[test]
fn a_popup_toggled_from_another_tab_gives_the_focus_back_to_that_tab() {
    let mut st = State::default();
    let (s, w1) = st.create_session(None, None).unwrap();
    add(&mut st, &s, &w1, "block:a", None, None);
    add(&mut st, &s, &w1, "block:pop", Some("pop"), Some("block:a"));
    st.float_block(&s, "block:pop", Bounds::default()).unwrap();
    st.toggle_overlay(&s, "pop").unwrap(); // hidden, made in tab 1

    let w2 = st.new_window(&s, None).unwrap();
    add(&mut st, &s, &w2, "block:c", None, None);
    assert_eq!(st.session(&s).unwrap().active, 1, "working in tab 2 on c");

    st.toggle_overlay(&s, "pop").unwrap(); // the key, pressed in tab 2: shown, focused
    assert_eq!(st.session(&s).unwrap().windows[0].focus.as_deref(), Some("block:pop"));
    st.toggle_overlay(&s, "pop").unwrap(); // the key again: hidden

    let ses = st.session(&s).unwrap();
    let focused = ses.windows[ses.active].focus.clone();
    assert_eq!(focused.as_deref(), Some("block:c"), "focus back to the pane the key was pressed in");
}
