//! A terminal's program as a process: its resume command, whether it reads a line, hanging it up
//! and killing it, and waiting for and reaping it.

use super::*;

/// A resume command, how many times one was registered, and whether a restore typed this one.
#[derive(Default)]
pub(crate) struct Registered {
    command: Option<Vec<String>>,
    generation: u64,
    offered: bool,
}

impl Terminal {
    pub fn resume(&self) -> Option<Vec<String>> {
        lock(&self.resume).command.clone()
    }

    pub fn set_resume(&self, command: Option<Vec<String>>) {
        let mut r = lock(&self.resume);
        *r = Registered { command, generation: r.generation + 1, offered: false };
    }

    /// Which registration is current, for `mark_resume_offered`.
    pub fn resume_generation(&self) -> u64 {
        lock(&self.resume).generation
    }

    /// A restore typed registration `generation` at the prompt: it isn't saved again, so the next
    /// restart doesn't offer it a second time. A registration made since is left alone.
    pub fn mark_resume_offered(&self, generation: u64) {
        let mut r = lock(&self.resume);
        if r.generation == generation {
            r.offered = true;
        }
    }

    /// The command to save: none once a restore has offered it.
    pub fn resume_to_save(&self) -> Option<Vec<String>> {
        let r = lock(&self.resume);
        r.command.clone().filter(|_| !r.offered)
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

/// A program started but not yet handed to its exit thread. Dropped on an error or a panic after
/// the spawn, it kills the program's group, reaps it and lets go of the PTY, so a failed start
/// leaves no program on a PTY nobody reads. `disarm` it once the exit thread owns these.
pub(super) struct Started(Option<(i32, Held)>);

impl Started {
    pub(super) fn new(pid: i32, held: Held) -> Started {
        Started(Some((pid, held)))
    }

    pub(super) fn disarm(mut self) {
        self.0 = None;
    }
}

impl Drop for Started {
    fn drop(&mut self) {
        let Some((pid, held)) = self.0.take() else { return };
        // SAFETY: only signals; unreaped, the program keeps its group ID ours.
        unsafe { libc::kill(-pid, libc::SIGKILL) };
        reap(pid);
        lock(&held).take(); // the reads end once nothing holds the PTY's other end
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
