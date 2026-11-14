use std::os::unix::fs::PermissionsExt;
use std::sync::Arc;
use std::time::Duration;

#[tokio::test]
async fn refuses_a_world_writable_runtime_dir() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o777)).unwrap();
    let socket = dir.path().join("server.sock");
    let info = leon_server::Info { pid: 1, version: "test", socket: socket.display().to_string() };
    let ctx = Arc::new(leon_server::Ctx::new(info, false));
    let opts = leon_server::serve::Options { socket, idle_exit: Duration::from_secs(60) };
    let r = tokio::time::timeout(Duration::from_secs(2), leon_server::serve::serve(ctx, opts)).await;
    assert!(matches!(r, Ok(Err(_))), "UNSAFE_DIR_ACCEPTED: server bound a socket in a 0777 dir");
}
