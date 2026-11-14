//! M5b criteria 5–7, 9, 14 (.agent/plans/2026-11-14-m5b-lua-runtime.md): `leon do SCRIPT|-e CODE`
//! through the real binary. Every script run has a deadline, so a script that never ends fails the
//! test instead of hanging it.

mod support;

use std::io::{BufRead, BufReader, Read};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use support::Env;

struct Done {
    code: i32,
    out: String,
    err: String,
    took: Duration,
}

/// Run `cmd` to the end, killing it (and failing) after `limit`.
fn finish(mut cmd: Command, limit: Duration) -> Done {
    let t = Instant::now();
    let child = cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
    wait(child, t, limit, String::new())
}

fn wait(mut child: Child, t: Instant, limit: Duration, mut out: String) -> Done {
    let mut stdout = child.stdout.take();
    let mut stderr = child.stderr.take().unwrap();
    let err = std::thread::spawn(move || {
        let mut s = String::new();
        let _ = stderr.read_to_string(&mut s);
        s
    });
    let rest = std::thread::spawn(move || {
        let mut s = String::new();
        if let Some(o) = stdout.as_mut() {
            let _ = o.read_to_string(&mut s);
        }
        s
    });
    let status = loop {
        if let Some(st) = child.try_wait().unwrap() {
            break st;
        }
        if t.elapsed() > limit {
            let _ = child.kill();
            let _ = child.wait();
            panic!("still running after {limit:?}");
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    out += &rest.join().unwrap();
    Done { code: status.code().unwrap_or(-1), out, err: err.join().unwrap(), took: t.elapsed() }
}

/// A server with no config and one session (whose block runs `sleep 300`).
fn setup() -> (Env, String, String) {
    let env = Env::new();
    let none = env.dir.path().join("no-init.lua");
    let out = env.cmd().env("LEON_CONFIG", &none).args(["server", "start"]).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let (s, b) = env.session();
    (env, s, b)
}

fn code(env: &Env, s: &str, src: &str, extra: &[&str]) -> Done {
    let mut c = env.cmd();
    c.args(["do", "-s", s, "-e", src]).args(extra);
    finish(c, Duration::from_secs(10))
}

/// Start `leon do` with stdout piped, and wait for it to print `ready`.
fn started(mut cmd: Command) -> (Child, mpsc::Receiver<String>) {
    let mut child = cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
    let stdout = child.stdout.take().unwrap();
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            let Ok(line) = line else { return };
            if tx.send(line).is_err() {
                return;
            }
        }
    });
    let first = rx.recv_timeout(Duration::from_secs(10));
    assert_eq!(first.as_deref(), Ok("ready"), "the script didn't start");
    (child, rx)
}

/// The rest of a started script's stdout and its exit status, within `limit`.
fn rest(child: Child, rx: mpsc::Receiver<String>, limit: Duration) -> Done {
    let t = Instant::now();
    let mut out = String::new();
    let mut child = child;
    let status = loop {
        while let Ok(l) = rx.try_recv() {
            out += &l;
            out.push('\n');
        }
        if let Some(st) = child.try_wait().unwrap() {
            break st;
        }
        if t.elapsed() > limit {
            let _ = child.kill();
            let _ = child.wait();
            panic!("still running after {limit:?}; stdout so far: {out}");
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    while let Ok(l) = rx.recv_timeout(Duration::from_millis(500)) {
        out += &l;
        out.push('\n');
    }
    let mut err = String::new();
    let _ = child.stderr.take().unwrap().read_to_string(&mut err);
    Done { code: status.code().unwrap_or(-1), out, err, took: t.elapsed() }
}

#[test]
fn c5_a_script_waits_for_child_exited_from_leon_run() {
    let (env, s, _) = setup();
    let dir = tempfile::tempdir().unwrap();
    let script = dir.path().join("wait.lua");
    std::fs::write(
        &script,
        "print('ready') io.stdout:flush()\n\
         local ev, ctx = leon.wait('child_exited', nil, 10)\n\
         print(ev.payload.exit_code, ev.type, ev.block_id ~= nil)\n",
    )
    .unwrap();
    let mut c = env.cmd();
    c.args(["do", "-s", &s, script.to_str().unwrap()]);
    let (child, rx) = started(c);
    env.ok(&["run", "-s", &s, "--", "sh", "-c", "exit 7"]);
    let d = rest(child, rx, Duration::from_secs(10));
    assert_eq!(d.code, 0, "{}", d.err);
    assert_eq!(d.out.trim(), "7\tchild_exited\ttrue", "{}", d.err);

    let d = code(&env, &s, "print(leon.wait('child_exited', nil, 0.2))", &[]);
    assert_eq!(d.code, 0, "{}", d.err);
    assert_eq!(d.out.trim(), "nil\ttimeout");
}

#[test]
fn c6_timers_stop_sleep_and_script_values() {
    let (env, s, b) = setup();
    let d = code(
        &env,
        &s,
        "local n = 0\n\
         local t\n\
         t = leon.every(0.05, function() n = n + 1; if n == 3 then t:cancel() end end)\n\
         leon.after(0.4, function() print(n, t:active()) end)\n",
        &[],
    );
    assert_eq!((d.code, d.out.trim()), (0, "3\tfalse"), "{}", d.err);

    assert_eq!(code(&env, &s, "leon.stop(3)", &[]).code, 3);
    assert_eq!(code(&env, &s, "leon.after(0.05, function() leon.stop(4) end)", &[]).code, 4);

    let d = code(&env, &s, "leon.sleep(0.1) print('slept')", &[]);
    assert_eq!(d.out.trim(), "slept", "{}", d.err);
    assert!(d.took >= Duration::from_millis(100), "{:?}", d.took);

    let src = format!(
        "print(leon.args.n, leon.args.s, leon.session_id == '{s}', leon.block_id == '{b}', \
         leon.json.encode(leon.json.decode('{{\"a\":1}}')))\n\
         local v = leon.call('session.view', {{ session_id = leon.session_id }})\n\
         print(v.session_id == leon.session_id, leon.session.view().session_id == leon.session_id)\n\
         local sz = leon.block.call('dev.leon.terminal', 'size')\n\
         print(type(sz))\n\
         leon.log('warn', 'careful now')\n"
    );
    let mut c = env.cmd();
    c.args(["do", "-s", &s, "-b", &b, "-e", &src, "n=3", "s=hi"]);
    let d = finish(c, Duration::from_secs(10));
    assert_eq!(d.code, 0, "{}", d.err);
    assert_eq!(d.out.trim(), "3\thi\ttrue\ttrue\t{\"a\":1}\ntrue\ttrue\ntable", "{}", d.err);
    assert!(d.err.contains("careful now"), "{}", d.err);
}

#[test]
fn c7_exec_captures_output_and_honours_timeouts() {
    let (env, s, _) = setup();
    let d = code(
        &env,
        &s,
        "local r = leon.exec({'sh', '-c', 'echo hi; exit 2'})\n\
         print(r.stdout == 'hi\\n', r.code, r.ok)\n\
         local r2 = leon.exec({'sleep', '5'}, { timeout = 0.2 })\n\
         print(r2.timed_out, r2.code)\n\
         local h = leon.exec.start({'cat'}, { stdin = true })\n\
         h:write('x') h:close_stdin()\n\
         print(h:wait(5).stdout)\n\
         print((pcall(leon.exec, 'echo hi')))\n",
        &[],
    );
    assert_eq!(d.code, 0, "{}", d.err);
    assert_eq!(d.out.trim(), "true\t2\tfalse\ntrue\t-1\nx\nfalse", "{}", d.err);
    assert!(d.took < Duration::from_secs(3), "took {:?}", d.took);
}

#[test]
fn c9_the_sandbox_has_no_popen_execute_or_exit() {
    let (env, s, _) = setup();
    for src in ["io.popen('ls')", "os.execute('true')", "os.exit(0)"] {
        let d = code(&env, &s, src, &[]);
        assert_eq!(d.code, 1, "{src} must fail: {} {}", d.out, d.err);
        assert!(d.err.contains("leon.exec") || d.err.contains("not available"), "{src}: {}", d.err);
    }

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("init.lua");
    std::fs::write(
        &path,
        "leon.action{ name = 'popen_it', title = 'P', run = function() io.popen('ls') end }\n\
         leon.action{ name = 'exit_it', title = 'E', run = function() os.exit(0) end }\n",
    )
    .unwrap();
    let env2 = Env::new();
    let out = env2.cmd().env("LEON_CONFIG", &path).args(["server", "start"]).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    for name in ["popen_it", "exit_it"] {
        let o = env2.run(&["do", name]);
        let err = String::from_utf8_lossy(&o.stderr);
        assert_eq!(o.status.code(), Some(1), "{name}: {err}");
        assert!(err.contains("leon.exec") || err.contains("not available"), "{name}: {err}");
    }
    env2.ok(&["ls"]);
}

#[test]
fn c14_handlers_keep_a_script_alive() {
    let (env, s, _) = setup();
    let mut c = env.cmd();
    c.args([
        "do",
        "-s",
        &s,
        "-e",
        "leon.on('child_exited', function(ctx, ev) print(ev.payload.exit_code, ctx.origin) leon.stop(0) end)\n\
         print('ready') io.stdout:flush()\n",
    ]);
    let (mut child, rx) = started(c);
    let t = Instant::now();
    while t.elapsed() < Duration::from_millis(300) {
        assert!(child.try_wait().unwrap().is_none(), "the script ended with a handler still registered");
        std::thread::sleep(Duration::from_millis(20));
    }
    env.ok(&["run", "-s", &s, "--", "sh", "-c", "exit 5"]);
    let d = rest(child, rx, Duration::from_secs(10));
    assert_eq!(d.code, 0, "{}", d.err);
    assert_eq!(d.out.trim(), "5\tscript", "{}", d.err);
}
