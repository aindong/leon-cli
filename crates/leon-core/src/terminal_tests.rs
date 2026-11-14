use std::sync::mpsc;

use super::cwd::hostname;
use super::*;

fn spawn(command: &[&str]) -> (Terminal, mpsc::Receiver<i32>) {
    let (tx, rx) = mpsc::channel();
    let spec = Spec { command: command.iter().map(|s| s.to_string()).collect(), cwd: None, env: vec![] };
    let tx = Mutex::new(tx);
    let t = Terminal::spawn(spec, move |n| {
        if let Notice::Exited(code) = n {
            lock(&tx).send(code).unwrap();
        }
    })
    .unwrap();
    (t, rx)
}

fn screen_until(t: &Terminal, pred: impl Fn(&str) -> bool) -> String {
    let end = std::time::Instant::now() + Duration::from_secs(10);
    loop {
        let text = t.text(true);
        if pred(&text) || std::time::Instant::now() > end {
            return text;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn runs_a_program_and_reads_its_screen() {
    let (t, rx) = spawn(&["sh", "-c", "read line; echo got-$line; exit 4"]);
    t.write(b"abc\r").unwrap();
    let text = screen_until(&t, |s| s.contains("got-abc"));
    assert!(text.lines().any(|l| l == "got-abc"), "{text:?}");
    assert_eq!(rx.recv_timeout(Duration::from_secs(10)).unwrap(), 4);
}

#[test]
fn writes_never_block_on_a_program_that_does_not_read() {
    // Raw mode: the line discipline queues input instead of dropping what doesn't fit.
    let (t, _rx) = spawn(&["sh", "-c", "stty raw -echo; echo ready; sleep 300"]);
    screen_until(&t, |s| s.contains("ready"));
    let t = Arc::new(t);
    let (done, finished) = mpsc::channel();
    let writer = t.clone();
    std::thread::spawn(move || {
        writer.write(&vec![b'x'; 256 << 10]).unwrap(); // far past the PTY's input buffer
        done.send(()).unwrap();
    });
    finished.recv_timeout(Duration::from_secs(5)).expect("write blocked on a full PTY");
}

#[test]
fn cwd_follows_the_shell_then_osc7_then_the_start_dir() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("sub")).unwrap();
    let start = std::fs::canonicalize(dir.path()).unwrap();
    let spec = Spec { command: vec!["sh".into()], cwd: Some(start.clone()), env: vec![] };
    let (tx, rx) = mpsc::channel();
    let tx = Mutex::new(tx);
    let t = Terminal::spawn(spec, move |n| {
        if let Notice::Exited(_) = n {
            lock(&tx).send(()).unwrap();
        }
    })
    .unwrap();
    assert_eq!(t.cwd(), Some(start.clone()));
    t.write(b"cd sub; echo cd-$((40+2))\r").unwrap();
    screen_until(&t, |s| s.lines().any(|l| l == "cd-42"));
    assert_eq!(t.cwd(), Some(start.join("sub")), "the shell's own cwd");
    t.write(b"printf '\\033]7;file://localhost/o%%20sc7\\007'; exit\r").unwrap();
    rx.recv_timeout(Duration::from_secs(10)).unwrap();
    assert_eq!(t.cwd(), Some(PathBuf::from("/o sc7")), "after exit: the last OSC 7 path");
    let spec = Spec { command: vec!["true".into()], cwd: Some(start.clone()), env: vec![] };
    let (done, exited) = mpsc::channel();
    let done = Mutex::new(done);
    let t = Terminal::spawn(spec, move |n| {
        if let Notice::Exited(_) = n {
            lock(&done).send(()).unwrap();
        }
    })
    .unwrap();
    exited.recv_timeout(Duration::from_secs(10)).unwrap();
    assert_eq!(t.cwd(), Some(start), "no process, no OSC 7: the start dir");
}

#[test]
fn the_exit_is_reported_after_the_last_output_is_on_the_screen() {
    // Small enough to sit in the read queue whole while the parser can't run.
    let script = "head -c 20000 /dev/zero | tr '\\0' y; printf END";
    let spec = Spec { command: vec!["sh".into(), "-c".into(), script.into()], cwd: None, env: vec![] };
    let (tx, exits) = mpsc::channel();
    let tx = Mutex::new(tx);
    let t = Terminal::spawn(spec, move |n| {
        if let Notice::Exited(_) = n {
            lock(&tx).send(()).unwrap();
        }
    })
    .unwrap();
    let parsing = lock(&t.term); // the parser blocks; the reader goes on queueing
    let end = std::time::Instant::now() + Duration::from_secs(10);
    while !t.has_exited() {
        assert!(std::time::Instant::now() < end, "the program didn't exit");
        std::thread::sleep(Duration::from_millis(5));
    }
    std::thread::sleep(DRAIN_WAIT + Duration::from_millis(300)); // past the EOF wait, too
    assert!(exits.try_recv().is_err(), "the exit was reported while its output was still unparsed");
    assert_eq!(t.output_bytes(), 0, "nothing parsed while the emulator was locked");
    drop(parsing);
    exits.recv_timeout(Duration::from_secs(10)).unwrap();
    assert!(t.text(true).ends_with("END"), "the last output is on the screen by the exit report");
    assert_eq!(t.output_bytes(), 20_003, "every byte counted");
}

#[test]
fn osc7_paths_count_only_from_this_host() {
    assert_eq!(pwd_path("file:///a%20b"), Some(PathBuf::from("/a b")));
    assert_eq!(pwd_path("file://localhost/x"), Some(PathBuf::from("/x")));
    let here = hostname().unwrap();
    assert_eq!(pwd_path(&format!("file://{here}/y")), Some(PathBuf::from("/y")));
    assert_eq!(pwd_path("file://some-remote-box.example/etc"), None, "an ssh session's path");
}

#[test]
fn dropping_hangs_up_the_program() {
    let (t, rx) = spawn(&["sleep", "300"]);
    drop(t);
    assert_eq!(rx.recv_timeout(Duration::from_secs(10)).unwrap(), 128 + libc::SIGHUP);
}

#[test]
fn the_line_editor_wait_needs_the_shell_in_front() {
    let (shell, _rx) = spawn(&["sh"]);
    assert!(shell.wait_line_editor(Duration::from_secs(10)), "an interactive shell reads a line");

    // A job in front with canonical mode off (a picker an rc file started) isn't the shell.
    let (picker, _rx) = spawn(&["sh", "-c", "set -m; sh -c 'stty -icanon; echo picker-up; sleep 300'"]);
    screen_until(&picker, |s| s.contains("picker-up"));
    assert!(!picker.wait_line_editor(Duration::from_millis(300)), "the job in front isn't the shell");

    let (shown, _rx) = {
        let (tx, rx) = mpsc::channel::<i32>();
        let spec = Spec { command: vec!["sleep".into(), "300".into()], cwd: None, env: vec![] };
        let tx = Mutex::new(tx);
        let t = Terminal::spawn_showing(spec, b"before\r\nthe program", move |n| {
            if let Notice::Exited(c) = n {
                let _ = lock(&tx).send(c);
            }
        });
        (t.unwrap(), rx)
    };
    assert_eq!(shown.text(true), "before\nthe program", "shown text is on the screen, not sent");
}
