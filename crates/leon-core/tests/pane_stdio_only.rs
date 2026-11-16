//! A pane's program starts as it always did (.agent/plans/2026-11-16-exec-failure.md): the PTY as
//! its stdio and controlling terminal, the server's environment with the spec's on top, its cwd,
//! and no other descriptor of the server's, not even one another thread holds open at the time.

use std::sync::mpsc;
use std::time::{Duration, Instant};

use leon_core::terminal::{Spec, Terminal};

/// Start `spec` and wait until its screen holds `done`; returns the screen.
fn screen(spec: Spec, input: &str, done: &str) -> String {
    let t = Terminal::spawn(spec, |_| {}).unwrap();
    if !input.is_empty() {
        t.write(input.as_bytes()).unwrap();
    }
    until(&t, done)
}

/// Wait until `t`'s screen holds `done`; returns the screen.
fn until(t: &Terminal, done: &str) -> String {
    let end = Instant::now() + Duration::from_secs(10);
    loop {
        let text = t.text(true);
        if text.contains(done) || Instant::now() > end {
            assert!(text.contains(done), "timed out waiting for {done:?}; screen:\n{text}");
            return text;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn sh(script: &str) -> Spec {
    Spec { command: vec!["sh".into(), "-c".into(), script.into()], cwd: None, env: vec![] }
}

#[test]
fn a_pane_sees_no_descriptor_another_thread_holds() {
    // a descriptor that is not close-on-exec, numbered far past the ones `ls` opens for itself
    let (made, planted) = mpsc::channel();
    let (release, held) = mpsc::channel::<()>();
    let holder = std::thread::spawn(move || {
        let mut pipe = [0; 2];
        // SAFETY: pipe fills the two descriptors; F_DUPFD copies one of ours.
        let fd = unsafe {
            assert_eq!(libc::pipe(pipe.as_mut_ptr()), 0);
            libc::fcntl(pipe[1], libc::F_DUPFD, 100)
        };
        made.send(fd).unwrap();
        let _ = held.recv();
        for fd in [pipe[0], pipe[1], fd] {
            // SAFETY: a descriptor this thread made.
            unsafe { libc::close(fd) };
        }
    });
    let planted = planted.recv().unwrap();
    assert!(planted >= 100, "{}", std::io::Error::last_os_error());

    let text = screen(sh("echo fds: $(ls /dev/fd) .; echo listed"), "", "listed");
    let line = text.lines().find(|l| l.starts_with("fds:")).unwrap();
    let fds: Vec<i32> = line.split_whitespace().filter_map(|fd| fd.parse().ok()).collect();
    assert!(fds.contains(&0) && fds.contains(&2), "the listing has the program's stdio: {line}");
    assert!(!fds.contains(&planted), "the pane inherited descriptor {planted}: {fds:?}");
    drop(release);
    holder.join().unwrap();
}

#[test]
fn a_pane_starts_as_before() {
    let dir = tempfile::tempdir().unwrap();
    let cwd = dir.path().canonicalize().unwrap();
    // SAFETY: set before this test starts any program; no other test reads it.
    unsafe { std::env::set_var("LEON_TEST_FROM_SERVER", "server") };
    let mut spec = sh(r#"echo "env=$LEON_TEST_FROM_SERVER/$LEON_TEST_SPEC shell=$SHELL"
echo "cwd=$(pwd -P)"
[ -t 0 ] && [ -t 1 ] && [ -t 2 ] && echo stdio=tty
(exec 3</dev/tty) 2>/dev/null && echo ctty=yes
echo "tty=$(tty)"
echo started; exec sleep 30"#);
    spec.cwd = Some(cwd.clone());
    spec.env = vec![("LEON_TEST_SPEC".into(), "spec".into())];
    let t = Terminal::spawn(spec, |_| {}).unwrap();
    let text = until(&t, "started");
    // SAFETY: getsid only reads; the pane's program is still running (sleep 30).
    let (sid, pid) = (unsafe { libc::getsid(t.pid()) }, t.pid());
    assert_eq!(sid, pid, "the program leads its own session");
    let shell = std::env::var("SHELL").unwrap_or_default();
    assert!(text.contains("env=server/spec shell="), "{text}");
    if !shell.is_empty() {
        assert!(text.contains(&format!("shell={shell}")), "SHELL is the server's: {text}");
    }
    assert!(text.contains(&format!("cwd={}", cwd.display())), "{text}");
    assert!(text.contains("stdio=tty"), "the PTY is stdin, stdout and stderr: {text}");
    assert!(text.contains("ctty=yes"), "the PTY is the controlling terminal: {text}");
    assert!(text.contains("tty=/dev/"), "`tty` names the PTY: {text}");
}

#[test]
fn the_default_program_is_a_login_shell_in_home() {
    let home = tempfile::tempdir().unwrap();
    let home_path = home.path().canonicalize().unwrap();
    let spec = Spec {
        command: vec![],
        cwd: None,
        env: vec![("SHELL".into(), "/bin/sh".into()), ("HOME".into(), home_path.display().to_string())],
    };
    let text = screen(spec, "echo \"argv0=$0 cwd=$(pwd -P)\"; echo sh''ell-done\n", "shell-done");
    assert!(text.contains(&format!("argv0=-sh cwd={}", home_path.display())), "{text}");
}

#[test]
fn the_whole_environment_is_the_servers_with_the_specs_on_top() {
    let dir = tempfile::tempdir().unwrap();
    let bin = dir.path().join("bin");
    std::fs::create_dir(&bin).unwrap();
    let out = dir.path().join("env");
    // a bare name found through the PATH the spec sets, as portable-pty looked it up
    let tool = bin.join("leon-test-dump-env");
    std::fs::write(&tool, format!("#!/bin/sh\nenv > {}\necho dumped\n", out.display())).unwrap();
    std::fs::set_permissions(&tool, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    let path = format!("{}:/usr/bin:/bin", bin.display());
    let spec = Spec {
        command: vec!["leon-test-dump-env".into()],
        cwd: Some(dir.path().into()),
        env: vec![("PATH".into(), path.clone()), ("LEON_TEST_SPEC".into(), "spec".into())],
    };
    screen(spec, "", "dumped");
    let got: std::collections::BTreeMap<String, String> = std::fs::read_to_string(&out)
        .unwrap()
        .lines()
        .filter_map(|l| l.split_once('=').map(|(k, v)| (k.to_string(), v.to_string())))
        .collect();
    let mut want: std::collections::BTreeMap<String, String> = std::env::vars().collect();
    want.insert("PATH".into(), path);
    want.insert("LEON_TEST_SPEC".into(), "spec".into());
    want.entry("SHELL".into()).or_insert_with(|| got["SHELL"].clone());
    // what sh sets or changes when it starts, and what macOS strips from a system binary's env;
    // values with newlines make extra lines in `env`'s output
    let compared = |(k, v): &(String, String)| {
        !["PWD", "OLDPWD", "SHLVL", "_"].contains(&k.as_str()) && !k.starts_with("DYLD_") && !v.contains('\n')
    };
    let got: std::collections::BTreeMap<_, _> = got.into_iter().filter(compared).collect();
    let want: std::collections::BTreeMap<_, _> = want.into_iter().filter(compared).collect();
    let differ: Vec<_> = want.keys().chain(got.keys()).filter(|k| want.get(*k) != got.get(*k)).collect();
    assert!(differ.is_empty(), "these differ from the server's env with the spec's on top: {differ:?}");
}
