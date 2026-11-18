//! A process's parent, group and terminal, read from the OS: the skill hook's foreground check,
//! and the server's permission guard (does a client run inside a leon block?).

pub struct Process {
    pub ppid: i32,
    pub pgid: i32,
    /// The foreground process group of its terminal; 0 or less without one.
    pub tpgid: i32,
}

/// How many generations `descends_from` climbs.
const MAX_DEPTH: usize = 64;

/// Whether `pid` is one of `roots` or runs under one of them.
pub fn descends_from(pid: i32, roots: &[i32]) -> bool {
    let mut pid = pid;
    for _ in 0..MAX_DEPTH {
        if pid <= 1 {
            return false;
        }
        if roots.contains(&pid) {
            return true;
        }
        match process(pid) {
            Some(p) if p.ppid != pid => pid = p.ppid,
            _ => return false,
        }
    }
    false
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_child_descends_from_us_and_we_not_from_it() {
        let mut child = std::process::Command::new("sleep").arg("5").spawn().unwrap();
        let (me, kid) = (std::process::id() as i32, child.id() as i32);
        assert!(descends_from(kid, &[me]));
        assert!(descends_from(me, &[me]));
        assert!(!descends_from(me, &[kid]));
        assert!(!descends_from(kid, &[]));
        child.kill().unwrap();
        child.wait().unwrap();
    }
}
