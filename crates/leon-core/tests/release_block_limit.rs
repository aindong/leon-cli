//! Release audit: a server holds at most 1024 blocks, the most a saved
//! state may have and still load. Before, the 1025th opened, the save went through, and the next
//! start refused the whole file, losing every session.

use std::sync::Arc;

use leon_core::terminal::{Spec, Terminal};
use leon_core::{Block, State, MAX_BLOCKS};

#[test]
fn the_block_past_the_limit_is_refused() {
    let terminal = Arc::new(
        Terminal::spawn(Spec { command: vec!["sleep".into(), "60".into()], cwd: None, env: vec![] }, |_| {}).unwrap(),
    );
    let mut st = State::default();
    let mut add = |i: usize| {
        let (s, w) = st.create_session(None, None).unwrap();
        let b = Block {
            id: format!("block:{i}"),
            label: None,
            window_id: w,
            keep_open: false,
            exit_code: None,
            terminal: terminal.clone(),
        };
        st.add_block(&s, b, None)
    };
    for i in 0..MAX_BLOCKS {
        add(i).unwrap_or_else(|e| panic!("block {i}: {e:?}"));
    }
    assert_eq!(add(MAX_BLOCKS).map_err(|e| e.status), Err(400), "a block past MAX_BLOCKS opened");
}
