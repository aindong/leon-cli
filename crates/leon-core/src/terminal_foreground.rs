//! The program in a terminal's foreground: the name of the process group leading its PTY, for the
//! app's icon per program. Only the name, never arguments or environment, and only for a process
//! the server's own user owns.

use super::*;

/// `pid`'s name, cleaned as a title is, if the server's user owns it; else None.
pub fn owned_process_name(pid: i32) -> Option<String> {
    ours((pid > 0).then(|| read(pid)).flatten())
}

/// The name of what `read` gave, cleaned, if the server's user owns it.
fn ours(read: Option<(u32, String)>) -> Option<String> {
    let (uid, name) = read?;
    // SAFETY: geteuid can't fail.
    (uid == unsafe { libc::geteuid() }).then(|| clean(&name)).filter(|n| !n.is_empty())
}

/// `pid`'s owner and name, if it exists and can be read.
#[cfg(target_os = "macos")]
fn read(pid: i32) -> Option<(u32, String)> {
    let size = std::mem::size_of::<libc::proc_bsdinfo>() as libc::c_int;
    // SAFETY: proc_pidinfo writes at most `size` bytes into our zeroed struct.
    let info = unsafe {
        let mut info: libc::proc_bsdinfo = std::mem::zeroed();
        let ptr = &mut info as *mut libc::proc_bsdinfo as *mut libc::c_void;
        (libc::proc_pidinfo(pid, libc::PROC_PIDTBSDINFO, 0, ptr, size) == size).then_some(info)?
    };
    // the longer name if set, else the 16-byte command name; both NUL-terminated in the struct
    let text = |raw: &[libc::c_char]| {
        let bytes: Vec<u8> = raw.iter().map(|&c| c as u8).take_while(|&b| b != 0).collect();
        String::from_utf8_lossy(&bytes).into_owned()
    };
    let name = Some(text(&info.pbi_name)).filter(|n| !n.is_empty()).unwrap_or_else(|| text(&info.pbi_comm));
    Some((info.pbi_uid, name))
}

/// The pids in process group `group`.
#[cfg(target_os = "macos")]
fn members(group: i32, _shell: i32) -> Vec<i32> {
    let mut pids = vec![0 as libc::pid_t; 64];
    let size = (pids.len() * std::mem::size_of::<libc::pid_t>()) as libc::c_int;
    // SAFETY: writes at most `size` bytes of pids into our buffer and returns how many pids.
    let n = unsafe { libc::proc_listpgrppids(group, pids.as_mut_ptr() as *mut libc::c_void, size) };
    pids.truncate(n.clamp(0, 64) as usize);
    pids
}

#[cfg(target_os = "linux")]
fn read(pid: i32) -> Option<(u32, String)> {
    use std::os::unix::fs::MetadataExt;
    // the owner and the name from one open, so a reused pid can't pair one process's with another's
    let mut f = std::fs::File::open(format!("/proc/{pid}/comm")).ok()?;
    let uid = f.metadata().ok()?.uid();
    let mut comm = Vec::new();
    f.read_to_end(&mut comm).ok()?;
    Some((uid, String::from_utf8_lossy(&comm).trim_end_matches('\n').to_string()))
}

/// The pids in process group `group` among `shell`'s descendants (a pipeline it, or a shell it
/// started, ran), looked at up to `MEMBERS_SEEN` of them.
#[cfg(target_os = "linux")]
fn members(group: i32, shell: i32) -> Vec<i32> {
    const MEMBERS_SEEN: usize = 512;
    let children = |pid: i32| {
        let list = std::fs::read_to_string(format!("/proc/{pid}/task/{pid}/children")).unwrap_or_default();
        list.split_whitespace().filter_map(|p| p.parse::<i32>().ok()).collect::<Vec<_>>()
    };
    let group_of = |pid: i32| {
        // `pid (comm) state ppid pgrp …`; comm may hold spaces and parens
        let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
        stat.get(stat.rfind(')')? + 2..)?.split(' ').nth(2)?.parse::<i32>().ok()
    };
    let (mut seen, mut next) = (Vec::new(), children(shell));
    while let Some(pid) = next.pop() {
        if seen.len() == MEMBERS_SEEN {
            break;
        }
        seen.push(pid);
        next.extend(children(pid));
    }
    seen.into_iter().filter(|&p| group_of(p) == Some(group)).collect()
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn read(_pid: i32) -> Option<(u32, String)> {
    None
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn members(_group: i32, _shell: i32) -> Vec<i32> {
    Vec::new()
}

impl Terminal {
    /// The name of the program in front: the PTY's foreground process group's leader (a shell's
    /// job, else the shell). None once the program exited, or for another user's (`sudo`).
    pub fn foreground(&self) -> Option<String> {
        let fd = lock(&self.master).as_raw_fd()?;
        let exited = lock(&self.exited);
        if *exited {
            return None;
        }
        // SAFETY: reads the foreground group of a PTY fd we own.
        let group = unsafe { libc::tcgetpgrp(fd) };
        if group <= 0 {
            return None;
        }
        // shortcut: a pid can exit and be reused between these reads, naming another process of
        // ours for one look; the owner check holds either way
        if let Some(leader) = read(group) {
            return ours(Some(leader)); // another user's leader (`sudo`) is nothing
        }
        // SAFETY: signal 0 only checks. EPERM: the leader is there but another user's, which
        // macOS won't let us read; nothing, as on Linux
        if unsafe { libc::kill(group, 0) } == -1 && io::Error::last_os_error().raw_os_error() == Some(libc::EPERM) {
            return None;
        }
        // the leader is gone (`cat f | less`): the newest member, usually the pipeline's last
        members(group, self.pid).into_iter().max().and_then(owned_process_name)
    }

    /// The foreground program as last reported (`check_foreground`): what a snapshot of the
    /// block gives, so it sits between the same events as the rest of the snapshot.
    pub fn reported_foreground(&self) -> Option<String> {
        lock(&self.reported).foreground.clone()
    }

    /// Report `foreground` as a notice if it changed since the last report. The server's tick is
    /// its one caller (two at once could send their notices in the other order), since a program
    /// that starts silently (`sleep`) prints nothing to notice.
    pub fn check_foreground(&self) {
        let now = self.foreground();
        {
            let mut r = lock(&self.reported);
            if r.foreground == now {
                return;
            }
            r.foreground.clone_from(&now);
        }
        (self.notify)(Notice::Foreground(now));
    }
}
