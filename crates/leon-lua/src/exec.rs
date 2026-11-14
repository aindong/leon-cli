//! `leon.exec` and `leon.exec.start`: run a program from an argv (never through a shell), capture
//! its output, feed its stdin, and stop it at a timeout (SIGTERM, then SIGKILL 2 s later). Inside
//! an action the timeout is at most the action's time left.
//!
//! Each program gets its own process group and signals go to the group, so a shell's children stop
//! with it. Output is read until the pipes close, but never past the deadline nor more than 2 s
//! after the program exits: a background child that keeps the pipe open can't stall the caller.

use std::io::{Read, Write};
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::process::{Child, ChildStdin, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use mlua::{IntoLuaMulti, Lua, MultiValue, Table, UserData, UserDataFields, UserDataMethods, Value};

use crate::api::flag;
use crate::runtime::{flags, rt};

/// Output kept per stream; the rest is read and dropped so the program never blocks on a full pipe.
const OUTPUT_CAP: usize = 16 << 20;
/// How long a program gets between SIGTERM and SIGKILL.
const KILL_GRACE: Duration = Duration::from_secs(2);
/// How long output is still read after the program exits.
const PIPE_GRACE: Duration = Duration::from_secs(2);

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

pub(crate) struct Proc {
    pid: i32,
    child: Mutex<Child>,
    status: Mutex<Option<ExitStatus>>,
    stdin: Mutex<Option<ChildStdin>>,
    out: Option<Arc<Pipe>>,
    err: Option<Arc<Pipe>>,
    /// `exec.start`'s `timeout` stopped it.
    timed_out: AtomicBool,
}

/// One output stream, filled by its reader thread.
#[derive(Default)]
struct Pipe {
    kept: Mutex<Vec<u8>>,
    closed: AtomicBool,
}

impl Proc {
    fn status(&self) -> Option<ExitStatus> {
        let mut st = lock(&self.status);
        if st.is_none() {
            *st = lock(&self.child).try_wait().ok().flatten();
        }
        *st
    }

    pub(crate) fn kill(&self, sig: i32) {
        if self.status().is_none() {
            // SAFETY: kill(2) on the process group our unreaped child leads (spawn sets
            // process_group(0)); while the leader isn't reaped its pgid can't be reused.
            unsafe { libc::kill(-self.pid, sig) };
        }
    }

    /// The exit status, or None if `until` passes first.
    fn wait_until(&self, until: Option<Instant>) -> Option<ExitStatus> {
        loop {
            if let Some(st) = self.status() {
                return Some(st);
            }
            if until.is_some_and(|u| Instant::now() >= u) {
                return None;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    /// Stop it: SIGTERM, then SIGKILL after the grace period.
    fn stop(&self) -> ExitStatus {
        self.kill(libc::SIGTERM);
        self.wait_until(Some(Instant::now() + KILL_GRACE)).unwrap_or_else(|| {
            self.kill(libc::SIGKILL);
            self.wait_until(None).expect("waits for ever")
        })
    }

    /// The result table. Output is read until the pipes close, `until`, or `PIPE_GRACE`.
    fn result(&self, lua: &Lua, st: ExitStatus, timed_out: bool, until: Option<Instant>) -> mlua::Result<Table> {
        let now = Instant::now();
        let cutoff = until.map_or(now + PIPE_GRACE, |u| u.max(now + Duration::from_millis(100)).min(now + PIPE_GRACE));
        let pipes = || self.out.iter().chain(&self.err);
        while !pipes().all(|p| p.closed.load(Ordering::SeqCst)) && Instant::now() < cutoff {
            std::thread::sleep(Duration::from_millis(5));
        }
        let timed_out = timed_out || self.timed_out.load(Ordering::SeqCst);
        let t = lua.create_table()?;
        let code = st.code().unwrap_or(-1);
        t.set("code", code)?;
        t.set("signal", st.signal())?;
        t.set("timed_out", timed_out)?;
        t.set("ok", code == 0 && !timed_out)?;
        for (k, p) in [("stdout", &self.out), ("stderr", &self.err)] {
            if let Some(p) = p {
                t.set(k, lua.create_string(&*lock(&p.kept))?)?;
            }
        }
        Ok(t)
    }
}

impl Drop for Proc {
    fn drop(&mut self) {
        if self.status().is_none() {
            self.kill(libc::SIGKILL);
            let _ = lock(&self.child).wait();
        }
    }
}

fn drain(mut r: impl Read + Send + 'static) -> Arc<Pipe> {
    let p = Arc::new(Pipe::default());
    let pipe = p.clone();
    std::thread::spawn(move || {
        let mut buf = [0u8; 8192];
        while let Ok(n @ 1..) = r.read(&mut buf) {
            let mut kept = lock(&pipe.kept);
            let room = OUTPUT_CAP.saturating_sub(kept.len());
            kept.extend_from_slice(&buf[..n.min(room)]);
        }
        pipe.closed.store(true, Ordering::SeqCst);
    });
    p
}

fn bad(msg: impl Into<String>) -> mlua::Error {
    mlua::Error::RuntimeError(msg.into())
}

/// Whether option `k` is `false` (don't capture / don't pass a pipe).
fn off(opts: Option<&Table>, k: &str) -> mlua::Result<bool> {
    Ok(matches!(opts.map(|o| o.get::<Value>(k)).transpose()?, Some(Value::Boolean(false))))
}

fn spawn(lua: &Lua, argv: Value, opts: Option<&Table>, start: bool) -> mlua::Result<Arc<Proc>> {
    let argv: Vec<String> = match &argv {
        Value::Table(t) => t.sequence_values::<String>().collect::<mlua::Result<_>>()?,
        _ => vec![],
    };
    let Some((prog, rest)) = argv.split_first() else {
        return Err(bad("leon.exec: argv must be a non-empty list of strings; leon runs no shell"));
    };
    let mut cmd = Command::new(prog);
    cmd.args(rest).process_group(0);
    let stdin = opts.map(|o| o.get::<Value>("stdin")).transpose()?.unwrap_or(Value::Nil);
    if let Some(o) = opts {
        if flag(o, "clear_env").map_err(|e| bad(e.0))? {
            cmd.env_clear();
        }
        if let Some(dir) = o.get::<Option<String>>("cwd")? {
            cmd.current_dir(dir);
        }
        if let Some(env) = o.get::<Option<Table>>("env")? {
            for pair in env.pairs::<String, Value>() {
                match pair? {
                    (k, Value::Boolean(false)) => cmd.env_remove(k),
                    (k, v) => cmd.env(k, v.to_string()?),
                };
            }
        }
    }
    let piped = |off: bool| if off { Stdio::null() } else { Stdio::piped() };
    cmd.stdout(piped(off(opts, "stdout")?)).stderr(piped(off(opts, "stderr")?));
    cmd.stdin(match &stdin {
        Value::String(_) => Stdio::piped(),
        Value::Boolean(true) if start => Stdio::piped(),
        Value::Nil | Value::Boolean(false) => Stdio::null(),
        v => return Err(bad(format!("leon.exec: stdin must be a string or true, got {}", v.type_name()))),
    });
    let mut child = cmd.spawn().map_err(|e| bad(format!("leon.exec: {prog}: {e}")))?;
    let mut pipe = child.stdin.take();
    if let Value::String(s) = &stdin {
        if let Some(mut w) = pipe.take() {
            let data = s.as_bytes().to_vec();
            std::thread::spawn(move || w.write_all(&data));
        }
    }
    let p = Arc::new(Proc {
        pid: child.id() as i32,
        out: child.stdout.take().map(drain),
        err: child.stderr.take().map(drain),
        child: Mutex::new(child),
        status: Mutex::new(None),
        stdin: Mutex::new(pipe),
        timed_out: AtomicBool::new(false),
    });
    let mut rt = rt(lua);
    rt.procs.retain(|w| w.strong_count() > 0);
    rt.procs.push(Arc::downgrade(&p));
    Ok(p)
}

/// The earlier of `secs` from now and the running action's deadline.
fn until(lua: &Lua, secs: Option<f64>) -> Option<Instant> {
    let t = secs.map(|s| Instant::now() + crate::runtime::secs(s));
    [t, flags(lua).deadline.get()].into_iter().flatten().min()
}

struct Handle(Arc<Proc>);

impl UserData for Handle {
    fn add_fields<F: UserDataFields<Self>>(fields: &mut F) {
        fields.add_field_method_get("pid", |_, h| Ok(h.0.pid));
    }

    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("wait", |lua, h, secs: Option<f64>| {
            let until = until(lua, secs);
            match h.0.wait_until(until) {
                Some(st) => h.0.result(lua, st, false, until)?.into_lua_multi(lua),
                None => (Value::Nil, "timeout").into_lua_multi(lua),
            }
        });
        methods.add_method("write", |_, h, data: mlua::LuaString| {
            let mut stdin = lock(&h.0.stdin);
            let w = stdin.as_mut().ok_or_else(|| bad("stdin is closed (start with stdin = true)"))?;
            w.write_all(&data.as_bytes()).map_err(|e| bad(format!("write: {e}")))
        });
        methods.add_method("close_stdin", |_, h, ()| {
            lock(&h.0.stdin).take();
            Ok(())
        });
        methods.add_method("kill", |_, h, sig: Option<Value>| {
            let sig = match sig {
                None => libc::SIGTERM,
                Some(Value::Integer(n)) => n as i32,
                Some(Value::Number(n)) => n as i32,
                Some(Value::String(s)) => match s.to_string_lossy().to_ascii_uppercase().trim_start_matches("SIG") {
                    "TERM" => libc::SIGTERM,
                    "KILL" => libc::SIGKILL,
                    "INT" => libc::SIGINT,
                    "HUP" => libc::SIGHUP,
                    other => return Err(bad(format!("unknown signal {other:?}"))),
                },
                Some(v) => return Err(bad(format!("signal must be a number or name, got {}", v.type_name()))),
            };
            h.0.kill(sig);
            Ok(())
        });
        methods.add_method("running", |_, h, ()| Ok(h.0.status().is_none()));
    }
}

pub(crate) fn add(lua: &Lua, leon: &Table) -> mlua::Result<()> {
    let exec = lua.create_table()?;
    exec.set(
        "start",
        lua.create_function(|lua, (argv, opts): (Value, Option<Table>)| {
            let timeout = opts.as_ref().map(|o| o.get::<Option<f64>>("timeout")).transpose()?.flatten();
            let p = spawn(lua, argv, opts.as_ref(), true)?;
            if let Some(end) = until(lua, timeout) {
                let w = Arc::downgrade(&p);
                std::thread::spawn(move || {
                    while let Some(p) = w.upgrade().filter(|p| p.status().is_none()) {
                        if Instant::now() >= end {
                            p.timed_out.store(true, Ordering::SeqCst);
                            p.stop();
                            return;
                        }
                        drop(p);
                        std::thread::sleep(Duration::from_millis(20));
                    }
                });
            }
            Ok(Handle(p))
        })?,
    )?;
    let meta = lua.create_table()?;
    meta.set(
        "__call",
        lua.create_function(|lua, (_, argv, opts): (Value, Value, Option<Table>)| -> mlua::Result<MultiValue> {
            let timeout = opts.as_ref().map(|o| o.get::<Option<f64>>("timeout")).transpose()?.flatten();
            let p = spawn(lua, argv, opts.as_ref(), false)?;
            let until = until(lua, timeout);
            let (st, timed_out) = match p.wait_until(until) {
                Some(st) => (st, false),
                None => (p.stop(), true),
            };
            p.result(lua, st, timed_out, until)?.into_lua_multi(lua)
        })?,
    )?;
    exec.set_metatable(Some(meta))?;
    leon.set("exec", exec)
}
