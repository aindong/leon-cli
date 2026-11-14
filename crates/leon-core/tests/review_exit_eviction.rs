use leon_core::terminal::{Spec, Terminal};
use leon_core::{Block, State};
use std::sync::Arc;

#[test]
fn kept_open_status_survives_other_exits() {
    let mut st = State::default();
    let (s, w) = st.create_session(None, None).unwrap();
    let spec = Spec { command: vec!["sleep".into(), "300".into()], cwd: None, env: vec![] };
    let terminal = Arc::new(Terminal::spawn(spec, |_| {}).unwrap());
    let b = Block { id: "block:kept".into(), label: None, window_id: w, keep_open: true, exit_code: None, terminal };
    st.add_block(&s, b).unwrap();
    st.block_exited("block:kept", 2);
    for i in 0..1100 {
        st.block_exited(&format!("block:other{i}"), 0);
    }
    assert_eq!(st.block("block:kept").unwrap().1.exit_code, Some(2));
    assert_eq!(st.exit_status("block:kept"), Some(2), "block.wait would hang on a kept-open exited block");
}
