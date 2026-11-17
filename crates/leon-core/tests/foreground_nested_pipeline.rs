//! A pipeline whose first program ended, started by a shell inside the pane's shell: its member is
//! found among the pane's shell's descendants, not only its children.
// Linux only: the fallback walks /proc; macOS asks for the whole group, which a_pipeline_whose_first_program_ended_reads_as_the_one_left covers.
#![cfg(target_os = "linux")]

use std::time::{Duration, Instant};

use leon_core::terminal::{Spec, Terminal};

fn until(t: &Terminal, what: &str, ok: impl Fn(&Terminal) -> bool) {
    let end = Instant::now() + Duration::from_secs(10);
    while !ok(t) {
        assert!(Instant::now() < end, "{what}: foreground is {:?}; screen:\n{}", t.foreground(), t.text(true));
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn a_nested_shells_pipeline_reads_as_the_one_left() {
    let spec = Spec { command: vec!["sh".into()], cwd: None, env: vec![("PS1".into(), "$ ".into())] };
    let t = Terminal::spawn(spec, |_| {}).unwrap();
    until(&t, "no prompt", |t| t.text(true).contains('$'));
    t.write(b"sh -i\n").unwrap(); // interactive: job control, so the pipeline gets its own group
    until(&t, "no inner prompt", |t| t.text(true).matches('$').count() >= 2);
    t.write(b"true | sleep 4321\n").unwrap();
    until(&t, "not sleep", |t| t.foreground().as_deref() == Some("sleep"));
    t.write(b"\x03").unwrap();
}
