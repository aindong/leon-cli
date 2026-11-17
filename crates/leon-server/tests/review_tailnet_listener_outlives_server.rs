//! Review proof (the skeptic, on serve over Tailscale): once `serve_on` returns, nothing listens
//! on the tailnet port any more, as for an app's embedded server, whose runtime lives on.

use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixListener;
use std::sync::Arc;
use std::time::{Duration, Instant};

const STATUS: &str = r#"{"BackendState":"Running","Self":{"ID":"nself","UserID":7,"TailscaleIPs":["127.0.0.1"]},"User":{"7":{"LoginName":"owner@example.com"}}}"#;
const WHOIS: &str = r#"{"Node":{"StableID":"npeer"},"UserProfile":{"ID":7,"LoginName":"owner@example.com"}}"#;

fn fake_localapi(path: &std::path::Path) {
    let l = UnixListener::bind(path).unwrap();
    std::thread::spawn(move || {
        for c in l.incoming() {
            let Ok(mut c) = c else { return };
            std::thread::spawn(move || {
                let mut req = Vec::new();
                let mut b = [0u8; 1];
                while !req.ends_with(b"\r\n\r\n") && req.len() < 8192 && c.read(&mut b).is_ok_and(|n| n == 1) {
                    req.push(b[0]);
                }
                let body = if String::from_utf8_lossy(&req).contains("/whois") { WHOIS } else { STATUS };
                let _ = c.write_all(format!("HTTP/1.0 200 OK\r\n\r\n{body}\n").as_bytes());
            });
        }
    });
}

#[tokio::test(flavor = "multi_thread")]
async fn no_tailnet_listener_outlives_serve_on() {
    let dir = tempfile::Builder::new().permissions(std::fs::Permissions::from_mode(0o700)).tempdir().unwrap();
    let api = dir.path().join("ts.sock");
    fake_localapi(&api);
    // the only test in this binary: no other test sees the seam
    std::env::set_var("LEON_TEST_TAILNET", serde_json::json!({ "localapi": api, "loopback": true }).to_string());
    let port = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
    let lua = dir.path().join("init.lua");
    std::fs::write(&lua, format!("leon.set(\"serve_tailscale\", true)\nleon.set(\"serve_tailscale_port\", {port})\n"))
        .unwrap();

    let socket = dir.path().join("server.sock");
    let info = leon_server::Info { pid: 1, version: "test", socket: socket.display().to_string() };
    let ctx = Arc::new(leon_server::Ctx::new(info, false));
    ctx.start_config(Some(lua));
    let bound = leon_server::serve::listen(&socket).await.unwrap();
    let opts = leon_server::serve::Options { socket: socket.clone(), idle_exit: Duration::from_secs(60) };
    let server = tokio::spawn(leon_server::serve::serve_on(ctx.clone(), bound, opts));

    let end = Instant::now() + Duration::from_secs(15);
    while ctx.tailnet.status().state != "serving" {
        assert!(Instant::now() < end, "never served: {:?}", ctx.tailnet.status());
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(std::net::TcpStream::connect(("127.0.0.1", port)).is_ok(), "listening while serving");

    ctx.shutdown.notify_one(); // what Embedded::stop does
    tokio::time::timeout(Duration::from_secs(10), server).await.unwrap().unwrap().unwrap();
    let end = Instant::now() + Duration::from_secs(2);
    while std::net::TcpStream::connect(("127.0.0.1", port)).is_ok() {
        assert!(Instant::now() < end, "the tailnet listener outlives the server on 127.0.0.1:{port}");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}
