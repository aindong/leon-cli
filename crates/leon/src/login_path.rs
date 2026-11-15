//! The login shell's PATH for the server's programs (docs/spec/cli.md: "Commands run with the login
//! shell's env"). Leon.app opened from Finder starts the server with launchd's bare PATH, and
//! `leon server proxy` over ssh with sshd's, so `leon run -- cargo test` found nothing. Only PATH is
//! taken: the rest of a login shell's environment is the shell's own business.

use std::io::Read;
use std::os::unix::process::CommandExt;
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

/// `$SHELL -l -c 'printenv PATH'`: the last line it prints, since a profile may print first. The
/// shell starts from the system PATH, so what comes back is what the profile builds, not ours again
/// (macOS's path_helper appends the PATH it was given).
fn login_shell_path() -> Option<String> {
    let shell = std::env::var_os("SHELL").filter(|s| !s.is_empty())?;
    let mut child = Command::new(shell)
        .args(["-l", "-c", "/usr/bin/printenv PATH"])
        .env("PATH", "/usr/bin:/bin:/usr/sbin:/sbin")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .process_group(0) // so a timeout kills whatever the profile started too
        .spawn()
        .ok()?;
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
    let path = text.lines().rev().find(|l| !l.trim().is_empty())?.trim().to_string();
    path.split(':').any(|d| d.starts_with('/')).then_some(path)
}

/// If every dir of the inherited PATH is in the login one (launchd's, sshd's), the login PATH in its
/// own order, as the user's terminal has it. Otherwise the server was started from a terminal with
/// a PATH of its own (a venv, an rc file's additions): keep it, and add what only the login has.
fn merge(inherited: &str, login: &str) -> String {
    let dirs = |p: &str| p.split(':').filter(|d| !d.is_empty()).map(String::from).collect::<Vec<_>>();
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
    }
}
