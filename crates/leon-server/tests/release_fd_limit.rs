//! Release audit: a server opens dozens of terminals even when its process started with a low
//! open-file limit. launchd starts the app, and so its embedded server, with a soft limit of 256;
//! each terminal holds three descriptors. This binary lowers its own limit, so it stays one test.

use std::sync::Arc;

use leon_server::{serve, Ctx, Info};
use serde_json::json;

#[tokio::test]
async fn a_low_fd_limit_still_opens_thirty_terminals() {
    let mut r = libc::rlimit { rlim_cur: 0, rlim_max: 0 };
    // SAFETY: getrlimit/setrlimit on our own process with a valid struct.
    unsafe {
        assert_eq!(libc::getrlimit(libc::RLIMIT_NOFILE, &mut r), 0);
        r.rlim_cur = 64;
        assert_eq!(libc::setrlimit(libc::RLIMIT_NOFILE, &r), 0);
    }
    let dir = tempfile::tempdir().unwrap();
    let rt = dir.path().join("rt");
    std::fs::create_dir(&rt).unwrap();
    std::fs::set_permissions(&rt, std::os::unix::fs::PermissionsExt::from_mode(0o700)).unwrap();
    let _bound = serve::listen(&rt.join("server.sock")).await.unwrap();

    let c = Arc::new(Ctx::new(Info { pid: 1, version: "t", socket: "/nonexistent".into() }, false));
    let sleep = json!({ "block": { "options": { "command": ["sleep", "30"] } } });
    let mut opened = 0;
    let mut err = None;
    for _ in 0..30 {
        match c.call("session.create", json!({ "layout": sleep })).await {
            Ok(_) => opened += 1,
            Err(e) => {
                err = Some(e);
                break;
            }
        }
    }
    c.end_programs(std::time::Duration::from_millis(200)).await;
    assert_eq!(opened, 30, "only {opened} terminals opened: {err:?}");
}
