//! Starting a pane's program on its PTY the way portable-pty's `spawn_command` does, so that a
//! failed exec is an error to the caller. portable-pty closes every descriptor past stderr in the
//! child, std's exec-error pipe among them: when the exec failed, std's child could not report it,
//! aborted with "fatal runtime error" on the PTY, and the parent saw a program that started.
//! Here the descriptors are marked close-on-exec instead (`spawn::only_stdio`), and the child
//! execs itself, so a binary that isn't a program is "Exec format error" and not a script for
//! `/bin/sh` (which `execvp` makes of any file the kernel refuses).

use std::collections::BTreeMap;
use std::ffi::{CString, OsStr, OsString};
use std::fs::OpenOptions;
use std::io::{self, Read};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::process::CommandExt;
use std::path::{Component, Path, PathBuf};
use std::process::{Child, Command};

use portable_pty::CommandBuilder;

use super::Spec;
use crate::spawn;

/// What a script without a `#!` line runs with, as `execvp` does.
const SH: &str = "/bin/sh";
/// How much of a refused file is read to tell a script from a binary (bash's sample).
const SAMPLE: usize = 80;

/// The login shell `spec` runs when its command is empty: `$SHELL` (the spec's, else this
/// process's) if executable, else the passwd entry's.
pub(super) fn shell(spec: &Spec) -> String {
    builder(spec).get_shell()
}

/// portable-pty's view of the spec's environment, for its rules on SHELL, HOME and PATH.
fn builder(spec: &Spec) -> CommandBuilder {
    let mut b = CommandBuilder::new_default_prog();
    for (k, v) in &spec.env {
        b.env(k, v);
    }
    b
}

/// Start `spec` with the PTY slave at `tty` as its stdio and controlling terminal.
pub(super) fn start(spec: &Spec, tty: &Path) -> io::Result<Child> {
    let b = builder(spec);
    // portable-pty's cwd: the spec's, else HOME (the spec's, else this process's, else passwd's)
    let home = b.get_env("HOME").map(PathBuf::from).or_else(std::env::home_dir).unwrap_or_else(|| "/".into());
    let cwd = spec.cwd.clone().filter(|d| d.is_dir()).unwrap_or(home);
    let (path, argv0) = match spec.command.first() {
        None => {
            let shell = b.get_shell();
            let base = shell.rsplit('/').next().unwrap_or(&shell);
            let argv0 = format!("-{base}");
            (PathBuf::from(&shell), argv0)
        }
        Some(exe) => (resolve(exe, &cwd, b.get_env("PATH"))?, exe.clone()),
    };
    let args: Vec<&OsStr> = spec.command.iter().skip(1).map(OsStr::new).collect();
    // this process's environment, SHELL when it has none, then the spec's on top
    let mut env: BTreeMap<OsString, OsString> = std::env::vars_os().collect();
    if let Some(shell) = b.get_env("SHELL") {
        env.insert("SHELL".into(), shell.into());
    }
    env.extend(spec.env.iter().map(|(k, v)| (k.into(), v.into())));
    let envp: Vec<OsString> = env
        .into_iter()
        .map(|(mut k, v)| {
            k.push("=");
            k.push(v);
            k
        })
        .collect();

    let prog = cstr(path.as_os_str())?;
    let argv = Argv::new(std::iter::once(OsStr::new(&argv0)).chain(args.iter().copied()))?;
    let sh_argv = Argv::new([OsStr::new("sh"), path.as_os_str()].into_iter().chain(args.iter().copied()))?;
    let envp = Argv::new(envp.iter().map(OsString::as_os_str))?;
    let sh = cstr(OsStr::new(SH))?;
    let mut room = spawn::fd_room();

    let slave = OpenOptions::new().read(true).write(true).custom_flags(libc::O_NOCTTY).open(tty)?;
    let mut cmd = Command::new(&path);
    cmd.stdin(slave.try_clone()?).stdout(slave.try_clone()?).stderr(slave).current_dir(&cwd);
    // SAFETY: between fork and exec only async-signal-safe calls, on buffers made before the
    // fork; std has already put the slave on 0, 1 and 2 and changed to `cwd`.
    unsafe {
        cmd.pre_exec(move || {
            for sig in [libc::SIGCHLD, libc::SIGHUP, libc::SIGINT, libc::SIGQUIT, libc::SIGTERM, libc::SIGALRM] {
                libc::signal(sig, libc::SIG_DFL);
            }
            let empty: libc::sigset_t = std::mem::zeroed();
            libc::sigprocmask(libc::SIG_SETMASK, &empty, std::ptr::null_mut());
            if libc::setsid() == -1 || libc::ioctl(0, libc::TIOCSCTTY as _, 0) == -1 {
                return Err(io::Error::last_os_error());
            }
            spawn::only_stdio(&mut room);
            libc::execve(prog.as_ptr(), argv.ptrs(), envp.ptrs());
            let refused = io::Error::last_os_error();
            if refused.raw_os_error() == Some(libc::ENOEXEC) && is_script(&prog) {
                libc::execve(sh.as_ptr(), sh_argv.ptrs(), envp.ptrs());
            }
            Err(refused)
        });
    }
    cmd.spawn().map_err(|e| failed(&path, e))
}

/// portable-pty's lookup: a name starting with `.` or `..` from the cwd, another relative one
/// through PATH, a path as it is; a directory or a file without the exec bit is refused here.
fn resolve(exe: &str, cwd: &Path, path: Option<&OsStr>) -> io::Result<PathBuf> {
    let exe_path = Path::new(exe);
    let through_path = exe_path.is_relative()
        && !matches!(exe_path.components().next(), Some(Component::CurDir | Component::ParentDir));
    if through_path {
        let dirs = path.map(|p| std::env::split_paths(p).collect::<Vec<_>>()).unwrap_or_default();
        return dirs
            .iter()
            .map(|dir| cwd.join(dir).join(exe_path))
            .find(|c| !c.is_dir() && access(c, libc::X_OK))
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::NotFound, format!("{exe} isn't on PATH (or isn't executable)"))
            });
    }
    let full = cwd.join(exe_path);
    let why = if full.is_dir() {
        "it is a directory"
    } else if access(&full, libc::X_OK) {
        return Ok(full);
    } else if access(&full, libc::F_OK) {
        "it is not executable"
    } else {
        "it does not exist"
    };
    Err(io::Error::new(io::ErrorKind::InvalidInput, format!("{}: {why}", full.display())))
}

fn access(p: &Path, mode: libc::c_int) -> bool {
    // SAFETY: a NUL-terminated path, read only.
    cstr(p.as_os_str()).is_ok_and(|c| unsafe { libc::access(c.as_ptr(), mode) } == 0)
}

/// The caller's message for a failed exec: the program and the OS's reason, and for a script the
/// interpreter its `#!` line names, which is what the reason is about.
fn failed(path: &Path, e: io::Error) -> io::Error {
    let mut b = Vec::new();
    let _ = std::fs::File::open(path).and_then(|f| f.take(256).read_to_end(&mut b));
    let head = b.strip_prefix(b"#!").and_then(|b| {
        let line = b.split(|c| *c == b'\n').next()?;
        Some(String::from_utf8_lossy(line).split_whitespace().next()?.to_string())
    });
    let text = match head {
        Some(interp) => format!("{}: can't run its interpreter {interp}: {e}", path.display()),
        None => format!("{}: {e}", path.display()),
    };
    io::Error::new(e.kind(), text)
}

/// Whether the kernel's refused file is a script for `/bin/sh`: no NUL before its first newline
/// in the first `SAMPLE` bytes, the rule bash and zsh use. Runs in the forked child.
///
/// # Safety
/// Only open, read and close, on a stack buffer.
unsafe fn is_script(prog: &CString) -> bool {
    let fd = libc::open(prog.as_ptr(), libc::O_RDONLY | libc::O_CLOEXEC);
    if fd < 0 {
        return false;
    }
    let mut buf = [0u8; SAMPLE];
    let got = libc::read(fd, buf.as_mut_ptr().cast(), SAMPLE);
    libc::close(fd);
    got >= 0 && !buf[..got as usize].split(|c| *c == b'\n').next().unwrap_or(&[]).contains(&0)
}

fn cstr(s: &OsStr) -> io::Result<CString> {
    CString::new(s.as_bytes()).map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "a NUL byte in the command"))
}

/// A NUL-terminated array of C strings for execve, made before the fork.
struct Argv {
    _strings: Vec<CString>,
    ptrs: Vec<*const libc::c_char>,
}

// SAFETY: the pointers point into `_strings`, owned alongside and never changed.
unsafe impl Send for Argv {}
unsafe impl Sync for Argv {}

impl Argv {
    fn new<'a>(items: impl IntoIterator<Item = &'a OsStr>) -> io::Result<Argv> {
        let strings = items.into_iter().map(cstr).collect::<io::Result<Vec<_>>>()?;
        let ptrs = strings.iter().map(|s| s.as_ptr()).chain([std::ptr::null()]).collect();
        Ok(Argv { _strings: strings, ptrs })
    }

    fn ptrs(&self) -> *const *const libc::c_char {
        self.ptrs.as_ptr()
    }
}
