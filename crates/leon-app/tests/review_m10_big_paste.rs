//! Review (correctness): a request over the server's 1 MiB limit must not cut the app's UI
//! connection for good: the server closes it, and the next call goes on a fresh one.
use std::os::unix::fs::PermissionsExt;

use leon_app::embedded;
use leon_app::remote::Remote;
use serde_json::json;

#[test]
fn typing_still_works_after_a_paste_over_the_request_limit() {
    let dir = tempfile::Builder::new().permissions(std::fs::Permissions::from_mode(0o700)).tempdir().unwrap();
    let server = embedded::start_at(dir.path().join("server.sock")).unwrap();
    server.call("session.create", json!({})).unwrap();
    let app = Remote::connect(server.socket()).unwrap();
    let (sid, bid) = {
        let m = app.model();
        (m.sessions[0].id.clone(), m.sessions[0].blocks[0].id.clone())
    };
    app.write(&sid, &bid, &vec![b'x'; 2 << 20]);
    let after = app.call("server.version", json!({}));
    assert!(after.is_ok(), "the UI connection died after a big paste: {:?}", after.err().map(|e| e.message));
}
