//! Update criterion 24 (.agent/plans/2026-11-16-self-update-core.md): over HTTPS the server's
//! certificate is checked. A loopback TLS server with a certificate nobody vouches for is refused
//! with a line that says so.

mod release_kit;
mod update_kit;

use std::net::TcpStream;
use std::process::{Child, Command, Stdio};

use update_kit::{assert_refused, run, until, Install, Release};

/// `openssl s_server`, killed when the test ends.
struct Tls(Child);

impl Drop for Tls {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
fn a_certificate_nobody_vouches_for_is_refused() {
    let rel = Release::new();
    let leon = Install::cli();
    let before = leon.state();
    let (key, cert) = (leon.home.join("tls.key"), leon.home.join("tls.crt"));
    let o = Command::new("openssl")
        .args(["req", "-x509", "-newkey", "rsa:2048", "-nodes", "-days", "1", "-subj", "/CN=127.0.0.1", "-keyout"])
        .arg(&key)
        .arg("-out")
        .arg(&cert)
        .output()
        .expect("openssl is needed by this test");
    assert!(o.status.success(), "openssl req: {}", release_kit::text(&o));
    // a free port: bound, read, and closed again
    let port = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
    // -WWW serves the files of its directory, so with certificate checks off the update would work.
    // OpenSSL takes an address to listen on; LibreSSL (macOS's own openssl) only a port.
    let serve = |accept: String| {
        let mut server = Command::new("openssl");
        server.args(["s_server", "-accept", &accept, "-WWW", "-key"]).arg(&key).arg("-cert").arg(&cert);
        server.current_dir(&rel.dir).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
        Tls(server.spawn().unwrap())
    };
    let mut first = serve(format!("127.0.0.1:{port}"));
    std::thread::sleep(std::time::Duration::from_millis(500));
    let _server = match first.0.try_wait().unwrap() {
        Some(_) => serve(port.to_string()),
        None => first,
    };
    until("the TLS server to listen", 30, || TcpStream::connect(("127.0.0.1", port)).is_ok());

    let url = format!("https://127.0.0.1:{port}");
    for args in [&[][..], &["--check"]] {
        let o = run(leon.cmd_with(&url, &Install::seam(&rel)).arg("self-update").args(args), 60);
        assert_refused(&o, &["certificate", &format!("127.0.0.1:{port}")]);
    }
    leon.assert_unchanged(before, "a certificate nobody vouches for");
    // the server is real: the same request with the check turned off by hand gets the manifest
    let o = Command::new("curl").args(["-q", "-sS", "-k", &format!("{url}/latest.json")]).output().unwrap();
    assert_eq!(o.stdout, rel.read("latest.json"), "the TLS server serves the release: {}", release_kit::text(&o));
}
