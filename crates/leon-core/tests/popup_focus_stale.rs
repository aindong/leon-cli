//! A popup shown without a toggle (the switcher, a click) forgets the pane an earlier toggle
//! would have given the focus back to.

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
fn a_popup_shown_without_a_toggle_forgets_the_old_return_target() {
    let mut st = State::default();
    let (s, w) = st.create_session(None, None).unwrap();
    add(&mut st, &s, &w, "block:a", None, None);
    add(&mut st, &s, &w, "block:b", None, Some("block:a"));
    add(&mut st, &s, &w, "block:c", None, Some("block:b"));
    add(&mut st, &s, &w, "block:pop", Some("pop"), Some("block:c"));
    st.float_block(&s, "block:pop", Bounds::default()).unwrap();
    st.toggle_overlay(&s, "pop").unwrap(); // hidden
    st.focus_block(&s, "block:b").unwrap();
    st.toggle_overlay(&s, "pop").unwrap(); // shown by toggle from b
    st.toggle_overlay(&s, "pop").unwrap(); // hidden, focus back on b
    st.focus_block(&s, "block:c").unwrap();
    st.focus_block(&s, "block:pop").unwrap(); // shown by focus, not by a toggle
    st.toggle_overlay(&s, "pop").unwrap();
    let f = st.session(&s).unwrap().windows[0].focus.clone();
    assert_ne!(f.as_deref(), Some("block:b"), "STALE_BACK_TO");
}
