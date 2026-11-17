//! The program in a terminal's foreground: the name of the process group leading its PTY, never its
//! arguments or environment, and nothing for a process another user owns. `check_foreground`
//! reports it once per change.

use std::sync::mpsc;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use leon_core::terminal::{owned_process_name, Notice, Spec, Terminal};

fn shell() -> (Terminal, mpsc::Receiver<String>) {
    let (tx, rx) = mpsc::channel();
    let tx = Mutex::new(tx);
    let spec = Spec { command: vec!["sh".into()], cwd: None, env: vec![("PS1".into(), "$ ".into())] };
    let t = Terminal::spawn(spec, move |n| {
        if let Notice::Foreground(f) = n {
            let _ = tx.lock().unwrap().send(format!("{f:?}"));
        }
    })
    .unwrap();
    (t, rx)
}

/// Wait until the foreground reads `want`.
fn reads(t: &Terminal, want: Option<&str>) {
    let end = Instant::now() + Duration::from_secs(10);
    while t.foreground().as_deref() != want {
        assert!(Instant::now() < end, "foreground is {:?}, not {want:?}; screen:\n{}", t.foreground(), t.text(true));
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// The shell's own name once its prompt is up (macOS's `sh` starts bash; some Linux's is dash).
fn shell_name(t: &Terminal) -> String {
    let end = Instant::now() + Duration::from_secs(10);
    while !t.text(true).contains('$') {
        assert!(Instant::now() < end, "no prompt");
        std::thread::sleep(Duration::from_millis(20));
    }
    t.foreground().expect("the shell is in front")
}

#[test]
fn a_program_run_in_the_shell_then_the_shell_again() {
    let (t, rx) = shell();
    let sh = shell_name(&t);
    t.check_foreground();
    t.write(b"sleep 4321\n").unwrap();
    reads(&t, Some("sleep")); // the name only: never "sleep 4321"
    t.check_foreground();
    t.check_foreground(); // no change: no notice
    t.write(b"\x03").unwrap();
    reads(&t, Some(&sh));
    t.check_foreground();
    let got: Vec<String> = rx.try_iter().collect();
    let sh = format!("Some({sh:?})");
    assert_eq!(got, [sh.as_str(), "Some(\"sleep\")", sh.as_str()], "one notice per change");
    t.write(b"exit\n").unwrap();
    let end = Instant::now() + Duration::from_secs(10);
    while !t.has_exited() {
        assert!(Instant::now() < end, "the shell didn't exit");
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(t.foreground(), None, "an exited program has no foreground");
    t.check_foreground();
    assert_eq!(rx.recv_timeout(Duration::from_secs(5)).unwrap(), "None");
}

#[test]
fn a_pipeline_whose_first_program_ended_reads_as_the_one_left() {
    let (t, _rx) = shell();
    let sh = shell_name(&t);
    // `true` leads the group and is gone at once; `sleep` stays in front
    t.write(b"true | sleep 4321\n").unwrap();
    reads(&t, Some("sleep"));
    t.write(b"\x03").unwrap();
    reads(&t, Some(&sh));
}

#[test]
fn another_users_process_is_nothing() {
    // Linux lets anyone read pid 1's name, so there the None is the owner check's
    #[cfg(target_os = "linux")]
    assert!(std::fs::read("/proc/1/comm").is_ok());
    assert_eq!(owned_process_name(1), None, "pid 1 is root's");
    assert_eq!(owned_process_name(-1), None);
    assert_eq!(owned_process_name(0), None);
    let me = owned_process_name(std::process::id() as i32).expect("this test's own process is ours");
    assert!(me.starts_with("foreground"), "{me}");
}

// Linux only: macOS kills a copied system binary (its signature), and a process's name there
// can't be set any other way without building a program.
#[cfg(target_os = "linux")]
#[test]
fn a_name_with_hidden_characters_is_cleaned() {
    let d = tempfile::tempdir().unwrap();
    // a copy of sleep whose file name has a right-to-left override in it
    let bin = d.path().join("sl\u{202e}eep");
    std::fs::copy("/bin/sleep", &bin).unwrap();
    let (t, _rx) = shell();
    shell_name(&t);
    t.write(format!("'{}' 4321\n", bin.display()).as_bytes()).unwrap();
    reads(&t, Some("sleep"));
    t.write(b"\x03").unwrap();
}
