//! Review of M13: `report_status` takes a state word, never pairs that would reach a record other
//! than the root (`done:id=child`) or skip its checks (`blocked:title=…`).

use leon_core::terminal::{Spec, Terminal};

#[test]
fn a_state_smuggling_pairs_is_refused() {
    let spec = Spec { command: vec!["sleep".into(), "60".into()], cwd: None, env: vec![] };
    let t = Terminal::spawn(spec, |_| {}).unwrap();
    for bad in ["blocked:id=other", "done:id=child", "blocked:kind=auth", "clear:id=x", "Done", ""] {
        assert!(t.report_status(bad, None, None).is_err(), "{bad:?} was taken");
    }
    assert!(t.program_status().is_empty(), "nothing was recorded: {:?}", t.program_status());
    t.report_status("blocked", Some("Deploy"), None).unwrap();
    assert_eq!(t.program_status().into_iter().map(|(id, _)| id).collect::<Vec<_>>(), vec![None]);
}
