//! A program `leon.exec` starts gets its stdin, stdout and stderr and no other descriptor of
//! leon's. In the server other threads open panes while a config's action runs: a pane's PTY is
//! not close-on-exec between `openpty` and its marking, and the program would hold it (the pane
//! then never sees its own program exit) for as long as it ran.

use std::sync::mpsc;
use std::time::{Duration, Instant};

use leon_lua::{CallCtx, Host, Runtime};
use serde_json::{json, Value};

struct NoServer;

impl Host for NoServer {
    fn call(&mut self, _: &str, _: Value, _: Option<Instant>) -> Result<Value, String> {
        Err("no server".into())
    }
}

fn run(action: &str) -> Result<Value, String> {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("init.lua");
    std::fs::write(
        &path,
        r#"leon.action{ name = "fds", title = "f", run = function() return leon.exec({ "ls", "/dev/fd" }) end }
leon.action{ name = "missing", title = "m", run = function() return leon.exec({ "/no/such/program" }) end }
"#,
    )
    .unwrap();
    let (rt, _cfg) = Runtime::load(&path, Box::new(NoServer)).map_err(|e| e.to_string()).unwrap();
    rt.run_action(action, json!({}), &CallCtx::default(), Instant::now() + Duration::from_secs(30))
}

#[test]
fn an_exec_child_sees_no_descriptor_another_thread_holds() {
    // another thread makes a descriptor that is not close-on-exec and holds it; a number far
    // past the ones `ls` opens for itself
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

    let r = run("fds").unwrap();
    assert_eq!(r["ok"], true, "{r}");
    let fds: Vec<i32> = r["stdout"].as_str().unwrap().split_whitespace().filter_map(|fd| fd.parse().ok()).collect();
    assert!(fds.contains(&0) && fds.contains(&2), "the listing has the program's stdio: {r}");
    assert!(!fds.contains(&planted), "the program inherited descriptor {planted}: {fds:?}");
    drop(release);
    holder.join().unwrap();
}

#[test]
fn a_missing_program_is_still_an_error_to_the_caller() {
    let e = run("missing").unwrap_err();
    assert!(e.contains("leon.exec: /no/such/program: No such file or directory"), "{e}");
}
