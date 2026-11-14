//! Impact proof (.agent/plans/2026-11-14-m3-layout-events.md): a window's tree and the session's
//! block list never disagree, whatever mix of split, close, exit, move and swap runs. Emptied-window
//! checks read the tree, `wait`/positions read the list, so a mismatch would strand a block.

use std::sync::Arc;

use leon_core::layout::{Direction, Side};
use leon_core::terminal::{Spec, Terminal};
use leon_core::{Block, Place, State};

fn check(st: &State) {
    for s in &st.sessions {
        let mut in_trees: Vec<String> = Vec::new();
        for w in &s.windows {
            let leaves: Vec<String> = w.layout.as_ref().unwrap().leaves().into_iter().map(String::from).collect();
            let mut listed: Vec<String> =
                s.blocks.iter().filter(|b| b.window_id == w.id).map(|b| b.id.clone()).collect();
            let mut sorted = leaves.clone();
            sorted.sort();
            listed.sort();
            assert_eq!(sorted, listed, "window {} tree vs list", w.id);
            assert!(w.focus.as_ref().is_some_and(|f| leaves.contains(f)), "focus is a block of the window");
            assert!(w.zoomed.as_ref().is_none_or(|z| leaves.contains(z)));
            in_trees.extend(leaves);
        }
        assert_eq!(in_trees.len(), s.blocks.len(), "every block is in exactly one tree");
    }
}

#[test]
fn random_layout_changes_keep_the_tree_and_list_in_step() {
    let terminal = Arc::new(
        Terminal::spawn(Spec { command: vec!["sleep".into(), "60".into()], cwd: None, env: vec![] }, |_| {}).unwrap(),
    );
    let mut seed: u64 = 7;
    let mut rnd = |n: usize| {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (seed >> 33) as usize % n
    };
    let mut st = State::default();
    let (s, w) = st.create_session(None, None).unwrap();
    let block = |id: String, w: &str| Block {
        keep_open: id.len().is_multiple_of(2),
        id,
        label: None,
        window_id: w.into(),
        exit_code: None,
        terminal: terminal.clone(),
    };
    st.add_block(&s, block("block:0".into(), &w), None).unwrap();
    let mut ran = [0; 7];
    for i in 1..400 {
        let Ok(sess) = st.session(&s) else { break };
        let ids: Vec<String> = sess.blocks.iter().map(|b| b.id.clone()).collect();
        let pick = |k: usize| ids[k % ids.len()].clone();
        let op = rnd(7);
        match op {
            0 | 1 if ids.len() < 10 => {
                let anchor = pick(rnd(100));
                let win = st.block(&anchor).unwrap().1.window_id.clone();
                let side = if rnd(2) == 0 { Side::Before } else { Side::After };
                let place = Place { anchor, direction: Direction::Vertical, side, ratio: 0.5, focus: rnd(2) == 0 };
                st.add_block(&s, block(format!("block:{i}"), &win), Some(&place)).unwrap();
            }
            2 if ids.len() < 10 => {
                let win = st.new_window(&s, None).unwrap();
                st.add_block(&s, block(format!("block:{i}"), &win), None).unwrap();
            }
            3 if ids.len() > 1 => st.close_block(&s, &pick(rnd(100))).unwrap(),
            4 if ids.len() > 1 => st.block_exited(&pick(rnd(100)), 0),
            5 => {
                let (a, b) = (pick(rnd(100)), pick(rnd(100)));
                if a != b {
                    st.move_block(&s, &a, &b, Direction::Horizontal, Side::After, 0.3).unwrap();
                }
            }
            _ => {
                let (a, b) = (pick(rnd(100)), pick(rnd(100)));
                st.swap_blocks(&s, &a, &b).unwrap();
                st.zoom_block(&s, &a, Some(true)).unwrap();
            }
        }
        ran[op] += 1;
        st.take_events();
        check(&st);
    }
    assert_eq!(ran.iter().sum::<i32>(), 399, "the session lived through every step");
    assert!(ran.iter().all(|&n| n > 20), "every kind of change ran: {ran:?}");
}
