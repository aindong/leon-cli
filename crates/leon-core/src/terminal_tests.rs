use std::sync::mpsc;

use super::cwd::hostname;
use super::*;

thread_local! {
    /// Whether `spawn_showing` fails at its last step on this thread, and the pid it had started.
    static FAIL_LATE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static LATE_PID: std::cell::Cell<i32> = const { std::cell::Cell::new(0) };
}

pub(super) fn late_failure(pid: i32) -> io::Result<()> {
    if !FAIL_LATE.get() {
        return Ok(());
    }
    LATE_PID.set(pid);
    Err(io::Error::other("a late failure (test)"))
}

#[test]
fn a_late_failure_leaves_no_program() {
    FAIL_LATE.set(true);
    // the sleep is in the program's process group, as whatever a shell starts is
    let spec = Spec { command: vec!["sh".into(), "-c".into(), "sleep 30 & wait".into()], cwd: None, env: vec![] };
    let failed = Terminal::spawn(spec, |_| {});
    FAIL_LATE.set(false);
    assert!(failed.is_err(), "the injected failure is the caller's error");
    let pid = LATE_PID.get();
    assert!(pid > 0);
    // SAFETY: signal 0 only asks whether the process (or one in the group) exists.
    assert_ne!(unsafe { libc::kill(pid, 0) }, 0, "the program {pid} still runs, or was never reaped");
    let end = std::time::Instant::now() + Duration::from_secs(5);
    while unsafe { libc::kill(-pid, 0) } == 0 {
        assert!(std::time::Instant::now() < end, "a process of the program's group {pid} still runs");
        std::thread::sleep(Duration::from_millis(20));
    }
}

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
fn a_removed_cwd_is_not_reported() {
    let dir = tempfile::tempdir().unwrap();
    let start = std::fs::canonicalize(dir.path()).unwrap();
    std::fs::create_dir(start.join("gone")).unwrap();
    let spec = Spec { command: vec!["sh".into()], cwd: Some(start.clone()), env: vec![] };
    let t = Terminal::spawn(spec, |_| {}).unwrap();
    t.write(b"cd gone; echo in-$((40+2))\r").unwrap();
    screen_until(&t, |s| s.lines().any(|l| l == "in-42"));
    std::fs::remove_dir(start.join("gone")).unwrap();
    // Linux's /proc/<pid>/cwd then reads "<path> (deleted)", which is no directory to start in
    let cwd = t.cwd().unwrap();
    assert!(!cwd.to_string_lossy().ends_with(" (deleted)"), "reported {cwd:?}");
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
    // Small enough to sit in the read queue whole while the parser can't run. It starts on our
    // word, once the parser is held: a busy machine could otherwise parse it all first.
    let script = "stty -echo; echo ready; read go; head -c 20000 /dev/zero | tr '\\0' y; printf END";
    let spec = Spec { command: vec!["sh".into(), "-c".into(), script.into()], cwd: None, env: vec![] };
    let (tx, exits) = mpsc::channel();
    let tx = Mutex::new(tx);
    let t = Terminal::spawn(spec, move |n| {
        if let Notice::Exited(_) = n {
            lock(&tx).send(()).unwrap();
        }
    })
    .unwrap();
    screen_until(&t, |s| s.contains("ready"));
    let parsing = lock(&t.term); // the parser blocks; the reader goes on queueing
    let before = t.output_bytes();
    t.write(b"\r").unwrap();
    let end = std::time::Instant::now() + Duration::from_secs(10);
    while !t.has_exited() {
        assert!(std::time::Instant::now() < end, "the program didn't exit");
        std::thread::sleep(Duration::from_millis(5));
    }
    std::thread::sleep(DRAIN_WAIT + Duration::from_millis(300)); // past the EOF wait, too
    assert!(exits.try_recv().is_err(), "the exit was reported while its output was still unparsed");
    assert_eq!(t.output_bytes(), before, "nothing parsed while the emulator was locked");
    drop(parsing);
    exits.recv_timeout(Duration::from_secs(10)).unwrap();
    assert!(t.text(true).ends_with("END"), "the last output is on the screen by the exit report");
    assert_eq!(t.output_bytes() - before, 20_003, "every byte counted");
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
    let soon = |t: &Terminal, wait: Duration| {
        let end = std::time::Instant::now() + wait;
        while !t.reads_a_line() && std::time::Instant::now() < end {
            std::thread::sleep(Duration::from_millis(20));
        }
        t.reads_a_line()
    };
    // bash, not sh: Debian's sh (dash) has no line editor and reads in canonical mode
    let (shell, _rx) = spawn(&["bash", "--norc", "-i"]);
    assert!(soon(&shell, Duration::from_secs(10)), "an interactive shell reads a line");

    // A job in front with canonical mode off (a picker an rc file started) isn't the shell.
    let (picker, _rx) = spawn(&["sh", "-c", "set -m; sh -c 'stty -icanon; echo picker-up; sleep 300'"]);
    screen_until(&picker, |s| s.contains("picker-up"));
    assert!(!soon(&picker, Duration::from_millis(300)), "the job in front isn't the shell");
}

#[test]
fn shown_text_is_on_the_screen_before_the_program() {
    let spec = Spec { command: vec!["sleep".into(), "300".into()], cwd: None, env: vec![] };
    let t = Terminal::spawn_showing(spec, b"before\r\nthe program", |_| {}).unwrap();
    assert_eq!(t.text(true), "before\nthe program");
}

#[test]
fn the_output_written_just_before_exit_is_kept() {
    // Feeding a long restored scrollback holds back the first read, as a starved server does: the
    // program writes and exits before anything reads its PTY (macOS then drops it after ~0.6 s).
    let shown: String = (0..1_000_000).map(|i| format!("restored line {i}\r\n")).collect();
    let spec = Spec { command: vec!["printf".into(), "END".into()], cwd: None, env: vec![] };
    let (tx, exits) = mpsc::channel();
    let tx = Mutex::new(tx);
    let t = Terminal::spawn_showing(spec, shown.as_bytes(), move |n| {
        if let Notice::Exited(_) = n {
            lock(&tx).send(()).unwrap();
        }
    })
    .unwrap();
    exits.recv_timeout(Duration::from_secs(30)).unwrap();
    let text = t.text(true);
    assert!(text.ends_with("END"), "the program's output is gone: {:?}", &text[text.len() - 40..]);
}

#[test]
fn openpty_reissues_only_eredriveopen() {
    // portable-pty's `bail!("failed to openpty: {:?}", io::Error::last_os_error())`
    let failed = |code: i32| format!("failed to openpty: {:?}", io::Error::from_raw_os_error(code));
    assert!(start::reissue(&failed(-6)), "{}", failed(-6));
    for code in [libc::EMFILE, libc::ENXIO, libc::EAGAIN, -60] {
        assert!(!start::reissue(&failed(code)), "{}", failed(code));
    }
}

#[test]
fn with_no_eof_the_exit_report_waits_for_what_was_read() {
    // No EOF (the reader is behind, or a job holds the PTY): 10 bytes read, none parsed yet.
    let (_at_eof, eof) = mpsc::channel::<()>();
    let (parser, drained) = mpsc::channel::<()>();
    let (read, parsed) = (Arc::new(AtomicU64::new(10)), Arc::new(AtomicU64::new(0)));
    let (done, reported) = mpsc::channel();
    let (r, p) = (read.clone(), parsed.clone());
    std::thread::spawn(move || {
        parsed_by_then(&eof, &drained, &r, &p, Duration::from_millis(50));
        done.send(()).unwrap();
    });
    assert!(reported.recv_timeout(Duration::from_millis(500)).is_err(), "reported before the parse");
    read.store(20, Ordering::SeqCst); // read after the EOF wait ran out: not waited for
    parsed.store(10, Ordering::SeqCst);
    reported.recv_timeout(Duration::from_secs(10)).expect("still waiting once it was parsed");

    // A parser that ended has parsed all it ever will.
    let (_at_eof, eof) = mpsc::channel::<()>();
    let (_, drained) = mpsc::channel::<()>();
    parsed_by_then(&eof, &drained, &AtomicU64::new(10), &AtomicU64::new(0), Duration::from_millis(50));
    drop(parser);
}
