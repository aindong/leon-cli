//! M20b: resizing a drawer (`dock_block` on its own side, what the app's divider drag sends)
//! changes its size where it is in the dock list, so the drawer that spans the corner stays put.

use leon_core::layout::{Direction, Side};
use leon_core::overlays::DockSide;
use leon_core::terminal::{Spec, Terminal};
use leon_core::{Block, Place, State};
use std::sync::Arc;

fn block(st: &mut State, s: &str, w: &str, id: &str, beside: Option<&str>) {
    let spec = Spec { command: vec!["sleep".into(), "300".into()], cwd: None, env: vec![] };
    let terminal = Arc::new(Terminal::spawn(spec, |_| {}).unwrap());
    let b = Block { id: id.into(), label: None, window_id: w.into(), keep_open: true, exit_code: None, terminal };
    let place = beside.map(|a| Place {
        anchor: a.into(),
        direction: Direction::Horizontal,
        side: Side::After,
        ratio: 0.5,
        focus: false,
    });
    st.add_block(s, b, place.as_ref()).unwrap();
}

#[test]
fn resizing_a_drawer_keeps_its_place_in_the_list() {
    let mut st = State::default();
    let (s, w) = st.create_session(None, None).unwrap();
    block(&mut st, &s, &w, "block:a", None);
    block(&mut st, &s, &w, "block:b", Some("block:a"));
    block(&mut st, &s, &w, "block:c", Some("block:b"));
    st.dock_block(&s, "block:a", DockSide::Left, 0.25).unwrap();
    st.dock_block(&s, "block:b", DockSide::Bottom, 0.3).unwrap();
    st.dock_block(&s, "block:a", DockSide::Left, 0.4).unwrap();
    let docked = &st.session(&s).unwrap().overlays[&w].docked;
    let got: Vec<(&str, f64)> = docked.iter().map(|d| (d.block_id.as_str(), d.size)).collect();
    assert_eq!(got, [("block:a", 0.4), ("block:b", 0.3)]);
    assert_eq!(st.session(&s).unwrap().windows[0].focus.as_deref(), Some("block:a"), "and focused");
    // another side still moves it (to the end), as before
    st.dock_block(&s, "block:a", DockSide::Right, 0.2).unwrap();
    let docked = &st.session(&s).unwrap().overlays[&w].docked;
    assert_eq!(docked.iter().map(|d| d.block_id.as_str()).collect::<Vec<_>>(), ["block:b", "block:a"]);
}
