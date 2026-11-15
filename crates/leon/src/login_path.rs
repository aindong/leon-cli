//! The login shell's PATH for the server's programs (docs/spec/cli.md: "Commands run with the login
//! shell's env"). Leon.app opened from Finder starts the server with launchd's bare PATH, and
//! `leon server proxy` over ssh with sshd's, so `leon run -- cargo test` found nothing. Only PATH is
//! taken: the rest of a login shell's environment is the shell's own business.

use std::io::Read;
use std::os::unix::ffi::OsStringExt;
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

/// A login shell that hasn't printed its PATH by then is left behind (killed) and PATH is kept.
const WAIT: Duration = Duration::from_secs(2);

/// Set this process's PATH from the login shell's (see `merge`). Call before the server's threads
/// start (the one reading the shell only reads its pipe).
pub fn adopt() {
    let inherited = std::env::var("PATH").unwrap_or_default();
    if let Some(login) = login_shell_path() {
        std::env::set_var("PATH", merge(&inherited, &login));
    }
}

/// `$SHELL -i -l -c /usr/bin/env` (the account's shell when SHELL is unset): the last `PATH=` line,
/// since a profile may print first (`env` is there on NixOS too, `printenv` isn't). Interactive too, as the user's own tab is, so a PATH
/// set in ~/.zshrc or ~/.bashrc (nvm, most installers) counts. The shell starts from the system
/// PATH, so what comes back is what the profile builds, not ours again (macOS's path_helper
/// appends the PATH it was given).
fn login_shell_path() -> Option<String> {
    let shell = std::env::var_os("SHELL").filter(|s| !s.is_empty()).map(PathBuf::from).or_else(account_shell)?;
    let mut cmd = Command::new(shell);
    cmd.args(["-i", "-l", "-c", "/usr/bin/env"])
        .env("PATH", "/usr/bin:/bin:/usr/sbin:/sbin")
        // an rc line that runs leon must not start a server, whose own probe would run it again
        .env("LEON_AUTOSTART", "0")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    // A session of its own: no controlling terminal for an interactive shell to take (zsh -i in the
    // background of `leon server run`'s terminal is stopped by SIGTTOU), and its own process group,
    // so `killpg` reaches whatever the rc files started.
    // SAFETY: setsid is async-signal-safe
    unsafe { cmd.pre_exec(|| if libc::setsid() < 0 { Err(std::io::Error::last_os_error()) } else { Ok(()) }) };
    let mut child = cmd.spawn().ok()?;
    let pid = child.id() as i32;
    let out = child.stdout.take()?;
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut text = String::new();
        let read = out.take(1 << 20).read_to_string(&mut text);
        let _ = tx.send(read.map(|_| text));
    });
    let text = rx.recv_timeout(WAIT);
    // SAFETY: a signal to the process group this function started
    unsafe { libc::killpg(pid, libc::SIGKILL) };
    let _ = child.wait();
    let text = text.ok()?.ok()?;
    let path = text.lines().rev().find_map(|l| l.strip_prefix("PATH="))?.to_string();
    path.split(':').any(|d| d.starts_with('/')).then_some(path)
}

/// The login shell in the passwd entry.
fn account_shell() -> Option<PathBuf> {
    // SAFETY: getpwuid returns a pointer into static storage or null; it is read at once, before
    // any other getpw* call on this (the only) thread
    let shell = unsafe {
        let pw = libc::getpwuid(libc::getuid());
        if pw.is_null() || (*pw).pw_shell.is_null() {
            return None;
        }
        std::ffi::CStr::from_ptr((*pw).pw_shell).to_bytes().to_vec()
    };
    Some(PathBuf::from(std::ffi::OsString::from_vec(shell))).filter(|p| p.is_absolute())
}

/// If every dir of the inherited PATH is in the login one (launchd's, sshd's), the login PATH in its
/// own order, as the user's terminal has it. Otherwise the server was started from a terminal with
/// a PATH of its own (a venv, an rc file's additions): keep it, and add what only the login has.
fn merge(inherited: &str, login: &str) -> String {
    // absolute dirs only: an empty or relative entry is looked up in each program's own cwd
    let dirs = |p: &str| p.split(':').filter(|d| d.starts_with('/')).map(String::from).collect::<Vec<_>>();
    let (mine, theirs) = (dirs(inherited), dirs(login));
    if mine.iter().all(|d| theirs.contains(d)) {
        return theirs.join(":");
    }
    let extra = theirs.iter().filter(|d| !mine.contains(d)).cloned().collect::<Vec<_>>();
    [mine, extra].concat().join(":")
}

#[cfg(test)]
mod tests {
    use super::merge;

    #[test]
    fn a_bare_path_takes_the_login_order_and_a_richer_one_keeps_its_own() {
        let login = "/Users/me/.cargo/bin:/opt/homebrew/bin:/usr/bin:/bin:/usr/sbin:/sbin";
        assert_eq!(merge("/usr/bin:/bin:/usr/sbin:/sbin", login), login);
        assert_eq!(merge("", login), login);
        assert_eq!(
            merge("/venv/bin:/usr/bin:/bin", login),
            "/venv/bin:/usr/bin:/bin:/Users/me/.cargo/bin:/opt/homebrew/bin:/usr/sbin:/sbin"
        );
        assert_eq!(merge("/usr/bin::/bin", "/x:/usr/bin:/bin"), "/x:/usr/bin:/bin", "empty entries are dropped");
        assert_eq!(merge("/usr/bin:/bin", "bin:.:/x:/usr/bin:/bin"), "/x:/usr/bin:/bin", "so are relative ones");
    }
}
