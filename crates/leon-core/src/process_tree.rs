//! A process's parent, group and terminal, read from the OS: the skill hook's foreground check,
//! and the server's permission guard (does a client run inside a leon block?).

pub struct Process {
    pub ppid: i32,
    pub pgid: i32,
    /// The foreground process group of its terminal; 0 or less without one.
    pub tpgid: i32,
}

/// A climb up the parents gives up past this many: a loop, or a tree no shell makes.
const MAX_DEPTH: usize = 4096;

/// Whether `pid` runs inside one of the terminals whose programs (each the leader of its own
/// session, as a PTY's program is) are `roots`: it is one, runs under one, or shares a session with
/// one, as a job keeps doing after its parent exits and it is handed to pid 1. None when that can't
/// be told (the process is gone, or the climb never ends), which a caller must not take as "no".
pub fn inside(pid: i32, roots: &[i32]) -> Option<bool> {
    // SAFETY: getsid has no preconditions; -1 for a pid that is gone.
    let session = unsafe { libc::getsid(pid) };
    if session < 0 {
        return None;
    }
    if roots.contains(&pid) || roots.contains(&session) {
        return Some(true);
    }
    let mut pid = pid;
    for _ in 0..MAX_DEPTH {
        let parent = parent(pid)?;
        if roots.contains(&parent) {
            return Some(true);
        }
        if parent <= 1 || parent == pid {
            return Some(false);
        }
        pid = parent;
    }
    None
}

#[cfg(target_os = "macos")]
pub fn process(pid: i32) -> Option<Process> {
    let size = std::mem::size_of::<libc::proc_bsdinfo>() as libc::c_int;
    // SAFETY: proc_pidinfo writes at most `size` bytes into our zeroed struct.
    let info = unsafe {
        let mut info: libc::proc_bsdinfo = std::mem::zeroed();
        let ptr = &mut info as *mut libc::proc_bsdinfo as *mut libc::c_void;
        (libc::proc_pidinfo(pid, libc::PROC_PIDTBSDINFO, 0, ptr, size) == size).then_some(info)?
    };
    Some(Process { ppid: info.pbi_ppid as i32, pgid: info.pbi_pgid as i32, tpgid: info.e_tpgid as i32 })
}

/// `pid`'s parent. The short record, unlike `process`'s, can be read for another user's process
/// (root's `login` under a terminal app's shell).
#[cfg(target_os = "macos")]
fn parent(pid: i32) -> Option<i32> {
    let size = std::mem::size_of::<libc::proc_bsdshortinfo>() as libc::c_int;
    // SAFETY: proc_pidinfo writes at most `size` bytes into our zeroed struct.
    let info = unsafe {
        let mut info: libc::proc_bsdshortinfo = std::mem::zeroed();
        let ptr = &mut info as *mut libc::proc_bsdshortinfo as *mut libc::c_void;
        (libc::proc_pidinfo(pid, libc::PROC_PIDT_SHORTBSDINFO, 0, ptr, size) == size).then_some(info)?
    };
    Some(info.pbsi_ppid as i32)
}

#[cfg(not(target_os = "macos"))]
fn parent(pid: i32) -> Option<i32> {
    process(pid).map(|p| p.ppid)
}

#[cfg(target_os = "linux")]
pub fn process(pid: i32) -> Option<Process> {
    // `pid (comm) state ppid pgrp session tty_nr tpgid …`; comm may hold spaces and parens.
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let f: Vec<i32> =
        stat[stat.rfind(')')? + 2..].split(' ').skip(1).take(5).map(|n| n.parse().ok()).collect::<Option<_>>()?;
    Some(Process { ppid: f[0], pgid: f[1], tpgid: f[4] })
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
pub fn process(_pid: i32) -> Option<Process> {
    None
}

/// Whether `pid` runs in a session that one of `ended` led: what lives on of a closed terminal (a
/// job, whatever process group it moves to, even handed to pid 1). Its leader is gone, or is
/// still this process's child (it ignored the hang-up, and the kill comes later). Linux and macOS
/// don't hand out a pid still in use as a session or group id, so a live leader with that pid and
/// another parent leads a new session, not this one.
pub fn in_ended_session(pid: i32, ended: &[i32]) -> bool {
    // SAFETY: getsid has no preconditions; -1 for a pid that is gone.
    let session = unsafe { libc::getsid(pid) };
    session > 0 && ended.contains(&session) && (!lives(session) || parent(session) == Some(std::process::id() as i32))
}

/// Whether process `pid` exists.
fn lives(pid: i32) -> bool {
    // SAFETY: signal 0 only checks; EPERM means it lives but isn't ours.
    let sent = unsafe { libc::kill(pid, 0) } == 0;
    sent || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn our_own_session_has_not_ended() {
        // SAFETY: getsid(0) asks about this process.
        let sid = unsafe { libc::getsid(0) };
        let me = std::process::id() as i32;
        // its leader lives and isn't our child (or it is us): a session that hasn't ended
        assert_eq!(in_ended_session(me, &[sid]), sid != me && !lives(sid));
        assert!(!in_ended_session(me, &[]));
    }

    #[test]
    fn a_child_is_inside_us_and_we_not_inside_it() {
        let mut child = std::process::Command::new("sleep").arg("5").spawn().unwrap();
        let (me, kid) = (std::process::id() as i32, child.id() as i32);
        assert_eq!(inside(kid, &[me]), Some(true));
        assert_eq!(inside(me, &[me]), Some(true));
        assert_eq!(inside(me, &[kid]), Some(false));
        assert_eq!(inside(kid, &[]), Some(false));
        child.kill().unwrap();
        child.wait().unwrap();
        assert_eq!(inside(kid, &[me]), None, "a process that is gone can't be told");
    }

    #[test]
    fn an_orphaned_job_is_still_inside_by_its_session() {
        // our session's leader stands in for a pane's program; a job orphaned to pid 1 keeps it
        // SAFETY: getsid(0) is this process's session.
        let leader = unsafe { libc::getsid(0) };
        let out = std::process::Command::new("sh").args(["-c", "sleep 5 >/dev/null 2>&1 & echo $!"]).output().unwrap();
        let orphan: i32 = String::from_utf8_lossy(&out.stdout).trim().parse().unwrap();
        assert_eq!(inside(orphan, &[leader]), Some(true));
        assert_eq!(inside(orphan, &[]), Some(false), "handed to pid 1");
        // SAFETY: signals the sleep we started.
        unsafe { libc::kill(orphan, libc::SIGKILL) };
    }
}
