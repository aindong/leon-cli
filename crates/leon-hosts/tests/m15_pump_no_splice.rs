//! M15 (Linux, found by m16a_docker_sshd): copying a socket into a pipe must not hold the pipe.
//! `std::io::copy` splices on Linux, and a splice from a socket keeps the pipe locked while it
//! waits, so sshd's non-blocking read of `leon server proxy`'s stdout blocked uninterruptibly:
//! sshd stopped forwarding the client's request and both sides waited for ever.

use std::io::Write;
use std::os::fd::FromRawFd;
use std::os::unix::net::UnixStream;
use std::sync::mpsc;
use std::time::Duration;

#[test]
fn a_waiting_pump_leaves_the_pipe_readable() {
    let mut fds = [0; 2];
    // SAFETY: pipe writes two fds into our array.
    assert_eq!(unsafe { libc::pipe(fds.as_mut_ptr()) }, 0);
    // SAFETY: both fds are fresh and ours; each File owns one.
    let (mut reader, writer) = unsafe { (std::fs::File::from_raw_fd(fds[0]), std::fs::File::from_raw_fd(fds[1])) };
    // SAFETY: fcntl on a fd we own.
    unsafe { libc::fcntl(fds[0], libc::F_SETFL, libc::O_NONBLOCK) };
    let (mut ours, theirs) = UnixStream::pair().unwrap();
    std::thread::spawn(move || {
        let mut from = theirs;
        let mut to = writer;
        leon_hosts::pump(&mut from, &mut to);
    });
    // one byte through, so the pump has looped back to waiting on the socket
    ours.write_all(b"x").unwrap();
    let mut first = [0u8; 1];
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while std::io::Read::read(&mut reader, &mut first).map_or(true, |n| n == 0) {
        assert!(std::time::Instant::now() < deadline, "the pump passed nothing on");
        std::thread::sleep(Duration::from_millis(10));
    }
    std::thread::sleep(Duration::from_millis(50)); // and is back in its read

    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut buf = [0u8; 16];
        let _ = tx.send(std::io::Read::read(&mut reader, &mut buf).map_err(|e| e.kind()));
    });
    let read = rx.recv_timeout(Duration::from_secs(2));
    ours.write_all(b"x\n").unwrap(); // unblocks a splice if one held the pipe
    assert_eq!(read, Ok(Err(std::io::ErrorKind::WouldBlock)), "the empty pipe's reader must not wait");
}
