//! Review finding (correctness): with serve_web off, setting a certificate path changes nothing
//! served, so it must not rebind (which closes every tailnet connection).
mod support;
use std::io::Read;
use std::time::Duration;
use support::localapi::Serving;

#[test]
fn a_cert_path_with_serve_web_off_keeps_tailnet_connections() {
    let s = Serving::start("");
    let mut conn = s.admitted();
    let out = s.cmd().args(["config", "set", "serve_web_cert", "/nowhere/web.crt"]).output().unwrap();
    assert!(out.status.success(), "config set: {}", String::from_utf8_lossy(&out.stderr));
    conn.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    let mut b = [0u8; 1];
    let r = conn.read(&mut b);
    assert!(r.is_err(), "the tailnet connection was closed by a rebind: {r:?}");
}
