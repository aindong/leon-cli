//! A terminal's program as a process: its resume command, whether it reads a line, hanging it up
//! and killing it, and waiting for and reaping it.

use super::*;

impl Terminal {
    pub fn resume(&self) -> Option<Vec<String>> {
        lock(&self.resume).clone()
    }

    pub fn set_resume(&self, command: Option<Vec<String>>) {
        *lock(&self.resume) = command;
    }

    /// Whether the program reads a line itself now: it is the PTY's foreground process group (no
    /// job an rc file started is in front) and its line editor has turned canonical mode off.
    pub fn reads_a_line(&self) -> bool {
        let Some(fd) = lock(&self.master).as_raw_fd() else { return false };
        let exited = lock(&self.exited);
        // SAFETY: reads the termios and foreground group of a PTY fd we own into locals.
        !*exited
            && unsafe {
                let mut t: libc::termios = std::mem::zeroed();
                libc::tcgetattr(fd, &mut t) == 0 && t.c_lflag & libc::ICANON == 0 && libc::tcgetpgrp(fd) == self.pid
            }
    }

    /// Hang up a still-running program: its process group (portable-pty makes the child a session
    /// leader) and the terminal's foreground job (e.g. vim started from a shell). An exited
    /// program's group was hung up when it exited, and its ID may belong to someone else by now,
    /// so it gets nothing.
    pub fn hang_up(&self) {
        let fd = lock(&self.master).as_raw_fd();
        let exited = lock(&self.exited);
        if *exited || self.pid <= 0 {
            return;
        }
        // SAFETY: only signals; the held lock keeps the leader unreaped, so both IDs are still ours.
        unsafe {
            let fg = fd.map_or(-1, |fd| libc::tcgetpgrp(fd));
            if fg > 0 && fg != self.pid {
                libc::kill(-fg, libc::SIGHUP);
            }
            libc::kill(-self.pid, libc::SIGHUP);
        }
    }

    /// SIGKILL the program's process group if the program still runs.
    pub fn kill(&self) {
        kill_group(self.pid, &self.exited);
    }

    pub fn has_exited(&self) -> bool {
        *lock(&self.exited)
    }
}

pub(super) fn kill_group(pid: i32, exited: &Mutex<bool>) {
    let exited = lock(exited);
    if !*exited && pid > 0 {
        // SAFETY: only signals; the held lock keeps the group ID ours.
        unsafe { libc::kill(-pid, libc::SIGKILL) };
    }
}

impl Drop for Terminal {
    /// Hang up the program, and kill it if it still runs after a grace period.
    fn drop(&mut self) {
        if self.has_exited() {
            return;
        }
        self.hang_up();
        let (pid, exited) = (self.pid, self.exited.clone());
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_secs(2));
            kill_group(pid, &exited);
        });
    }
}

/// Block until `pid` exits, leaving it unreaped (a zombie keeps its process group ID taken).
pub(super) fn wait_exit(pid: i32) {
    loop {
        // SAFETY: waitid on our own child, writing into a local; WNOWAIT leaves it waitable.
        let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
        let r = unsafe { libc::waitid(libc::P_PID, pid as libc::id_t, &mut info, libc::WEXITED | libc::WNOWAIT) };
        if r == 0 || io::Error::last_os_error().kind() != io::ErrorKind::Interrupted {
            return;
        }
    }
}

/// Reap `pid` and turn its wait status into a shell-style exit status.
pub(super) fn reap(pid: i32) -> i32 {
    let mut status = 0;
    loop {
        // SAFETY: waitpid on our own child, writing into a local.
        let r = unsafe { libc::waitpid(pid, &mut status, 0) };
        if r == pid {
            break;
        }
        if r < 0 && io::Error::last_os_error().kind() != io::ErrorKind::Interrupted {
            return 1;
        }
    }
    if libc::WIFEXITED(status) {
        libc::WEXITSTATUS(status)
    } else if libc::WIFSIGNALED(status) {
        128 + libc::WTERMSIG(status)
    } else {
        1
    }
}
