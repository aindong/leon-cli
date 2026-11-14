use std::os::unix::fs::PermissionsExt;
use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;

#[tokio::test]
async fn oversized_request_line_is_refused_before_parsing() {
    let dir = tempfile::Builder::new().permissions(std::fs::Permissions::from_mode(0o700)).tempdir().unwrap();
    let socket = dir.path().join("server.sock");
    let info = leon_server::Info { pid: 1, version: "test", socket: socket.display().to_string() };
    let ctx = Arc::new(leon_server::Ctx::new(info, false));
    let opts =
        leon_server::serve::Options { socket: socket.clone(), idle_exit: Duration::from_secs(60), persist: None };
    let server = tokio::spawn(leon_server::serve::serve(ctx, opts));
    let end = std::time::Instant::now() + Duration::from_secs(10);
    let s = loop {
        if let Ok(s) = UnixStream::connect(&socket).await {
            break s;
        }
        assert!(!server.is_finished() && std::time::Instant::now() < end, "server did not come up");
        tokio::time::sleep(Duration::from_millis(20)).await;
    };
    let (mut r, mut w) = s.into_split();
    tokio::spawn(async move {
        let junk = vec![b'a'; 64 << 20];
        let _ = w.write_all(&junk).await;
        let _ = w.write_all(b"\n").await;
    });
    let mut buf = Vec::new();
    let _ = tokio::time::timeout(Duration::from_secs(30), r.read_to_end(&mut buf)).await;
    let reply = String::from_utf8_lossy(&buf);
    assert!(!reply.contains("expected value"), "LINE_NOT_BOUNDED: server buffered and parsed a 64 MiB line: {reply}");
}
