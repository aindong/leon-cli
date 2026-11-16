//! Starting another program so that it gets its stdin, stdout and stderr and no other descriptor
//! of this process.
//!
//! A descriptor is not always close-on-exec from birth: macOS makes a pipe (a socket, an accepted
//! connection) first and marks it second, and a PTY is opened first and marked second on Linux
//! too. A program forked by another thread between the two keeps the descriptor for as long as it
//! runs: a pane's PTY (the pane then never sees its own program exit), or the pipe
//! `Command::spawn` reads until its child has started (that `spawn` then never returns).

use std::os::unix::process::CommandExt;
use std::process::Command;

/// Have `cmd`'s child mark every descriptor past stderr close-on-exec before its exec. With a
/// `pre_exec`, std forks and execs where it could have used `posix_spawn`.
pub fn stdio_only(cmd: &mut Command) {
    let mut room = fd_room();
    // SAFETY: between fork and exec only `only_stdio`, which is async-signal-safe.
    unsafe {
        cmd.pre_exec(move || {
            only_stdio(&mut room);
            Ok(())
        });
    }
}

/// Room for `only_stdio`'s list (macOS): the whole descriptor table, not a likely count, since
/// an older kernel lists only the numbers below the room it is given. Pages never written cost
/// nothing.
pub(crate) fn fd_room() -> Vec<[i32; 2]> {
    // SAFETY: getdtablesize has no preconditions.
    let table = if cfg!(target_os = "macos") { unsafe { libc::getdtablesize() } } else { 0 };
    vec![[0; 2]; table.max(0) as usize]
}

/// The slow way stops at this number: a container's limit can be a billion, and the child would
/// spend minutes inside `spawn`.
const SLOW_MOST: libc::c_int = 1 << 16;

/// For a forked child, before exec: mark every descriptor past stderr close-on-exec. Marked, not
/// closed: `spawn`'s own pipe still has to report a failed exec. `room` holds the list of
/// descriptors on macOS (a `proc_fdinfo` each), made before the fork since nothing may allocate
/// after it.
///
/// # Safety
/// Only async-signal-safe calls: proc_pidinfo (one syscall) and getpid, or on Linux the
/// close_range syscall, then `mark_table`'s.
pub(crate) unsafe fn only_stdio(room: &mut [[i32; 2]]) {
    #[cfg(target_os = "macos")]
    let marked = {
        let size = std::mem::size_of_val(room).min(libc::c_int::MAX as usize) as libc::c_int;
        let got = libc::proc_pidinfo(libc::getpid(), libc::PROC_PIDLISTFDS, 0, room.as_mut_ptr().cast(), size);
        let listed = got > 0 && got < size; // not when the list filled its room: it may be cut short
        if listed {
            for [fd, _kind] in &room[..got as usize / std::mem::size_of::<[i32; 2]>()] {
                if *fd > 2 {
                    libc::fcntl(*fd, libc::F_SETFD, libc::FD_CLOEXEC);
                }
            }
        }
        listed
    };
    // the flag is Linux 5.11's: an older kernel refuses it
    #[cfg(target_os = "linux")]
    let marked = {
        let _ = &room;
        let (first, last, flags) = (3, libc::c_uint::MAX as libc::c_long, libc::CLOSE_RANGE_CLOEXEC as libc::c_long);
        libc::syscall(libc::SYS_close_range, first as libc::c_long, last, flags) == 0
    };
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    let marked = {
        let _ = &room;
        false
    };
    if !marked {
        mark_table();
    }
}

/// The slow way, when the fast one is refused (no list on macOS, Linux before 5.11, another
/// unix): every number the table holds, 10 ms and more under a high limit.
///
/// # Safety
/// Only fcntl and getdtablesize, both async-signal-safe.
unsafe fn mark_table() {
    // shortcut: a descriptor numbered past SLOW_MOST stays inheritable (it takes that many open
    // at once, or a dup2 to a high number); list them some other way if that ever happens
    for fd in 3..libc::getdtablesize().min(SLOW_MOST) {
        libc::fcntl(fd, libc::F_SETFD, libc::FD_CLOEXEC);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_descriptor_is_marked_the_fast_way_and_the_slow_way() {
        for slow in [false, true] {
            // SAFETY: a copy of stdin (not close-on-exec, as a dup is), marked, read back, closed.
            unsafe {
                let fd = libc::dup(0);
                assert!(fd > 2, "{}", std::io::Error::last_os_error());
                if slow {
                    mark_table();
                } else {
                    only_stdio(&mut fd_room());
                }
                assert_ne!(libc::fcntl(fd, libc::F_GETFD) & libc::FD_CLOEXEC, 0, "the slow way: {slow}");
                libc::close(fd);
            }
        }
    }
}
