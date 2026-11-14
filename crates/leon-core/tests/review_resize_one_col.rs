//! Review proof: a terminal resized to one column survives a wide char.

use std::time::{Duration, Instant};

use leon_core::terminal::{Spec, Terminal};

#[test]
fn one_column_terminal_survives_a_wide_char() {
    let cmd = "sleep 0.5; printf '\\344\\270\\255\\n'; echo done; sleep 5";
    let spec = Spec { command: vec!["sh".into(), "-c".into(), cmd.into()], cwd: None, env: vec![] };
    let t = Terminal::spawn(spec, |_| {}).unwrap();
    t.resize(10, 1).unwrap();
    let end = Instant::now() + Duration::from_secs(5);
    loop {
        let flat = t.text(true).replace('\n', "");
        if flat.contains("done") {
            return;
        }
        assert!(Instant::now() < end, "BUG: reader thread died on a wide char at 1 column: {flat:?}");
        std::thread::sleep(Duration::from_millis(20));
    }
}
