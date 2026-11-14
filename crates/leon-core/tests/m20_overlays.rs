use leon_core::overlays::{Bounds, DockSide};
use leon_core::terminal::{Spec, Terminal};
use leon_core::{Block, State};
use std::sync::Arc;

fn state() -> (State, String, String) {
    let mut st = State::default();
    let (s, w) = st.create_session(None, None).unwrap();
    let terminal = Arc::new(
        Terminal::spawn(Spec { command: vec!["sleep".into(), "300".into()], cwd: None, env: vec![] }, |_| {}).unwrap(),
    );
    st.add_block(
        &s,
        Block {
            id: "block:a".into(),
            label: Some("scratch".into()),
            window_id: w.clone(),
            keep_open: true,
            exit_code: None,
            terminal,
        },
        None,
    )
    .unwrap();
    (st, s, w)
}

#[test]
fn float_only_window_toggle_dock_unfloat_and_close() {
    let (mut st, s, w) = state();
    let terminal = st.block("block:a").unwrap().1.terminal.clone();
    st.float_block(&s, "block:a", Bounds::default()).unwrap();
    assert!(st.session(&s).unwrap().windows[0].layout.is_none());
    assert_eq!(st.session(&s).unwrap().overlays[&w].floating[0].block_id, "block:a");
    st.toggle_overlay(&s, "scratch").unwrap();
    assert!(st.session(&s).unwrap().overlays[&w].floating[0].hidden);
    assert!(st.session(&s).unwrap().windows[0].focus.is_none());
    assert!(Arc::ptr_eq(&terminal, &st.block("block:a").unwrap().1.terminal));
    st.toggle_overlay(&s, "scratch").unwrap();
    st.dock_block(&s, "block:a", DockSide::Left, 0.25).unwrap();
    assert!(st.session(&s).unwrap().overlays[&w].floating.is_empty());
    st.unfloat_block(&s, "block:a").unwrap();
    assert!(st.session(&s).unwrap().overlays[&w].docked.is_empty());
    assert_eq!(st.session(&s).unwrap().windows[0].layout.as_ref().unwrap().leaves(), vec!["block:a"]);
    st.float_block(&s, "block:a", Bounds::default()).unwrap();
    st.close_block(&s, "block:a").unwrap();
    assert!(st.sessions.is_empty());
}

#[test]
fn bounds_are_finite_and_contained() {
    for b in [
        Bounds { x: -0.1, ..Bounds::default() },
        Bounds { width: 0., ..Bounds::default() },
        Bounds { x: 0.9, width: 0.5, ..Bounds::default() },
        Bounds { height: f64::NAN, ..Bounds::default() },
    ] {
        assert!(b.check().is_err());
    }
}

fn second(st: &mut State, s: &str, w: &str, id: &str) {
    let terminal = st.block("block:a").unwrap().1.terminal.clone();
    let anchor = st
        .session(s)
        .unwrap()
        .windows
        .iter()
        .find(|win| win.id == w)
        .unwrap()
        .layout
        .as_ref()
        .map(|t| t.id().to_string());
    let place = anchor.map(|anchor| leon_core::Place {
        anchor,
        direction: leon_core::layout::Direction::Horizontal,
        side: leon_core::layout::Side::After,
        ratio: 0.5,
        focus: true,
    });
    st.add_block(
        s,
        Block {
            id: id.into(),
            label: Some(id.into()),
            window_id: w.into(),
            keep_open: true,
            exit_code: None,
            terminal,
        },
        place.as_ref(),
    )
    .unwrap();
}

#[test]
fn z_order_dock_collision_move_close_and_swap_keep_unique_placement() {
    use leon_core::layout::{Direction, Side};
    let (mut st, s, w) = state();
    second(&mut st, &s, &w, "block:b");
    second(&mut st, &s, &w, "block:c");
    st.float_block(&s, "block:a", Bounds::default()).unwrap();
    st.float_block(&s, "block:b", Bounds::default()).unwrap();
    st.focus_block(&s, "block:a").unwrap();
    assert_eq!(st.session(&s).unwrap().overlay(&w).floating.last().unwrap().block_id, "block:a");
    st.dock_block(&s, "block:b", DockSide::Right, 0.3).unwrap();
    let before = st.session(&s).unwrap().overlay(&w);
    assert!(st.dock_block(&s, "block:a", DockSide::Right, 0.3).is_err());
    assert_eq!(st.session(&s).unwrap().overlay(&w), before);
    assert!(st.move_block(&s, "block:c", "block:a", Direction::Horizontal, Side::After, 0.5).is_err());
    st.swap_blocks(&s, "block:a", "block:c").unwrap();
    assert_eq!(st.session(&s).unwrap().overlay(&w).floating[0].block_id, "block:c");
    st.close_block(&s, "block:a").unwrap();
    assert_eq!(st.session(&s).unwrap().blocks.len(), 2, "closing last tile retains overlays");
    st.unfloat_block(&s, "block:c").unwrap();
    st.move_block(&s, "block:b", "block:c", Direction::Horizontal, Side::After, 0.5).unwrap();
    assert!(st.session(&s).unwrap().overlay(&w).ids().next().is_none());
    assert_eq!(st.session(&s).unwrap().windows[0].layout.as_ref().unwrap().leaves().len(), 2);
}

#[test]
fn m20_showing_an_overlay_activates_its_window() {
    let (mut st, s, w) = state();
    st.float_block(&s, "block:a", Bounds::default()).unwrap();
    st.toggle_overlay(&s, "scratch").unwrap();
    st.new_window(&s, None).unwrap();
    st.take_events();
    st.toggle_overlay(&s, "scratch").unwrap();
    let session = st.session(&s).unwrap();
    assert_eq!(session.windows[session.active].id, w);
    assert!(st.take_events().iter().any(|e| e.body.to_string().contains("active_window_changed")));
}
