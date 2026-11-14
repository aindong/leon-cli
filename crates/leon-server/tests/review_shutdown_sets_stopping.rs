//! Review proof: once the socket loop has shut down in-process (the app quitting), an open
//! connection can't create a session whose programs nothing would end.

use std::os::unix::fs::PermissionsExt;
use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;

#[tokio::test]
async fn no_session_is_created_after_an_in_process_shutdown() {
    let dir = tempfile::Builder::new().permissions(std::fs::Permissions::from_mode(0o700)).tempdir().unwrap();
    let socket = dir.path().join("server.sock");
    let info = leon_server::Info { pid: 1, version: "test", socket: socket.display().to_string() };
    let ctx = Arc::new(leon_server::Ctx::new(info, false));
    let bound = leon_server::serve::listen(&socket).await.unwrap();
    let opts = leon_server::serve::Options { socket: socket.clone(), idle_exit: Duration::from_secs(60) };
    let server = tokio::spawn(leon_server::serve::serve_on(ctx.clone(), bound, opts));
    let (r, mut w) = UnixStream::connect(&socket).await.unwrap().into_split();
    let mut r = BufReader::new(r);
    let mut line = String::new();
    w.write_all(b"{\"id\":1,\"method\":\"server.version\"}\n").await.unwrap();
    tokio::time::timeout(Duration::from_secs(10), r.read_line(&mut line)).await.unwrap().unwrap();
    ctx.shutdown.notify_one(); // what Embedded::stop does when the app quits
    tokio::time::timeout(Duration::from_secs(10), server).await.unwrap().unwrap().unwrap();
    line.clear();
    let create = b"{\"id\":2,\"method\":\"session.create\",\"params\":{\"layout\":{\"block\":{\"options\":{\"command\":[\"/bin/sleep\",\"30\"]}}}}}\n";
    w.write_all(create).await.unwrap();
    tokio::time::timeout(Duration::from_secs(10), r.read_line(&mut line)).await.unwrap().unwrap();
    let leaked = ctx.state().sessions.len();
    ctx.end_programs(Duration::from_millis(200)).await; // clean up whatever leaked
    assert_eq!(leaked, 0, "SESSION_AFTER_SHUTDOWN: {line}");
}
