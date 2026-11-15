//! Update criterion 13 (.agent/plans/2026-11-16-self-update-core.md): before the first release the
//! manifest's address answers 404, which reads as "no release yet", not as a failure. A server
//! error, no connection, a name that does not resolve and a stalled answer each give one line.

mod release_kit;
mod update_kit;

use serde_json::json;
use update_kit::{assert_refused, run, Install, Mode, Release, OLD};

#[test]
fn a_404_for_the_manifest_reads_as_no_release_yet() {
    let rel = Release::new();
    let (srv, leon) = (rel.serve(), Install::cli());
    let before = leon.state();
    srv.set("latest.json", Mode::Status(404));
    for args in [&[][..], &["--check"]] {
        let o = leon.update(&rel, &srv, args);
        assert_eq!(o.code, 0, "the same status as up to date: {}", o.text());
        assert!(o.out.contains("no release yet"), "it says so: {}", o.text());
        assert_eq!(o.err, "", "and nothing else");
    }
    leon.assert_unchanged(before, "no release yet");
}

#[test]
fn a_server_error_is_one_line() {
    let rel = Release::new();
    let (srv, leon) = (rel.serve(), Install::cli());
    srv.set("latest.json", Mode::Status(500));
    assert_refused(&leon.update(&rel, &srv, &[]), &["answered 500"]);
    assert_refused(&leon.update(&rel, &srv, &["--check"]), &["answered 500"]);
}

#[test]
fn no_connection_is_one_line() {
    let rel = Release::new();
    let leon = Install::cli();
    // a port nothing listens on: bound, read, and closed again
    let port = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
    let url = format!("http://127.0.0.1:{port}");
    let o = run(leon.cmd_with(&url, &Install::seam(&rel)).arg("self-update"), 60);
    assert_refused(&o, &["could not connect", &format!("127.0.0.1:{port}")]);
}

#[test]
fn a_name_that_does_not_resolve_is_one_line() {
    let rel = Release::new();
    let leon = Install::cli();
    // .invalid never resolves (RFC 6761), so this asks the resolver and connects to nothing
    let o =
        run(leon.cmd_with("https://leon-update-test.invalid/releases", &Install::seam(&rel)).arg("self-update"), 60);
    assert_refused(&o, &["could not find", "leon-update-test.invalid"]);
}

#[test]
fn an_answer_that_stalls_times_out_in_one_line() {
    let rel = Release::new();
    let (srv, leon) = (rel.serve(), Install::cli());
    srv.set("latest.json", Mode::Stall);
    let seam = json!({ "keys": rel.key.keys_file, "version": OLD, "timeout_secs": 2 });
    let o = run(leon.cmd_with(&srv.url(), &seam).arg("self-update"), 30);
    assert_refused(&o, &["timed out", "latest.json"]);
}
