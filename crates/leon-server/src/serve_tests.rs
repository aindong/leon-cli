use super::*;

#[tokio::test]
async fn bind_refuses_an_open_dir_and_leaves_it_alone() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(bind(&dir.path().join("server.sock")).await.is_err());
    let mode = std::fs::metadata(dir.path()).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o755, "LEON_RUNTIME_DIR=/tmp must not chmod /tmp");
}

#[tokio::test]
async fn bind_makes_a_new_dir_private_and_the_socket_0600() {
    let parent = tempfile::tempdir().unwrap();
    let socket = parent.path().join("leon/server.sock");
    let _l = bind(&socket).await.unwrap();
    let mode = |p: &Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode(&parent.path().join("leon")), 0o700);
    assert_eq!(mode(&socket), 0o600);
}
