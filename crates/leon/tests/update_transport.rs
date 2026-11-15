//! Update criterion 11 (.agent/plans/2026-11-16-self-update-core.md): releases come over HTTPS.
//! Plain HTTP is for a loopback address only (what these tests use), on the first request and on
//! every redirect. `LEON_RELEASE_URL` points leon at another place for releases.

mod release_kit;
mod update_kit;

use update_kit::{assert_refused, run, Install, Mode, Release, VERSION};

#[test]
fn plain_http_to_another_machine_is_refused_before_connecting() {
    let rel = Release::new();
    let leon = Install::cli();
    let before = leon.state();
    let elsewhere = ["http://example.invalid/leon", "http://192.0.2.1/leon", "http://localhost.example.invalid/leon"];
    // names that only start like loopback; in the last, what is before the @ is a user name
    for url in
        elsewhere.into_iter().chain(["http://127.0.0.1.example.invalid/leon", "http://127.0.0.1@example.invalid/leon"])
    {
        // 192.0.2.1 is TEST-NET-1: a connection to it would hang, and the 10 s deadline would trip
        let o = run(leon.cmd_with(url, &Install::seam(&rel)).arg("self-update"), 10);
        assert_refused(&o, &["plain http", url]);
    }
    for url in ["ftp://127.0.0.1/leon", "file:///tmp/leon", "127.0.0.1/leon"] {
        let o = run(leon.cmd_with(url, &Install::seam(&rel)).arg("self-update"), 10);
        assert_refused(&o, &["https", url]);
    }
    leon.assert_unchanged(before, "a release address that is not https");
    assert_eq!(leon.curl_ran(), Vec::<String>::new(), "curl never ran: each was refused before any connection");
}

#[test]
fn a_redirect_to_plain_http_elsewhere_is_refused() {
    let rel = Release::new();
    let (srv, leon) = (rel.serve(), Install::cli());
    let before = leon.state();
    srv.set("latest.json", Mode::Redirect("http://192.0.2.1/latest.json".into()));
    let o = run(leon.cmd(&rel, &srv).arg("self-update"), 10);
    assert_refused(&o, &["plain http", "http://192.0.2.1/latest.json"]);
    // and for a release file, after a good manifest
    srv.clear("latest.json");
    srv.set(&update_kit::name("cli"), Mode::Redirect("http://192.0.2.1/leon.tar.gz".into()));
    let o = run(leon.cmd(&rel, &srv).arg("self-update"), 10);
    assert_refused(&o, &["plain http", "http://192.0.2.1/leon.tar.gz"]);
    leon.assert_unchanged(before, "a redirect to plain http");
    let ran = leon.curl_ran();
    assert!(
        !ran.is_empty() && ran.iter().all(|c| !c.contains("192.0.2.1")),
        "curl never ran for that address: {ran:?}"
    );
}

#[test]
fn a_redirect_that_stays_on_loopback_is_followed() {
    let rel = Release::new();
    let (srv, leon) = (rel.serve(), Install::cli());
    std::fs::create_dir(rel.dir.join("moved")).unwrap();
    for file in ["latest.json", "latest.json.minisig"] {
        std::fs::copy(rel.dir.join(file), rel.dir.join("moved").join(file)).unwrap();
        srv.set(file, Mode::Redirect(format!("{}/moved/{file}", srv.url())));
    }
    let o = leon.update(&rel, &srv, &[]);
    assert_eq!(o.code, 0, "the update goes through: {}", o.text());
    assert_eq!(leon.version(), format!("leon {VERSION} (fake)"));
    assert!(srv.asked().contains(&"moved/latest.json".to_string()), "it followed the redirect: {:?}", srv.asked());
    let ran = leon.curl_ran();
    assert!(ran.len() >= 5, "curl fetched the manifest, its signature and the file: {ran:?}");
    for line in &ran {
        let words: Vec<&str> = line.split_whitespace().collect();
        assert!(words.contains(&"--proto"), "every request names the protocols it allows: {line}");
        assert!(!words.contains(&"-k") && !words.contains(&"--insecure"), "certificate checks stay on: {line}");
        assert!(!words.contains(&"-L") && !words.contains(&"--location"), "leon follows redirects itself: {line}");
    }
}

#[test]
fn redirects_that_never_end_are_given_up_on() {
    let rel = Release::new();
    let (srv, leon) = (rel.serve(), Install::cli());
    srv.set("latest.json", Mode::Redirect(format!("{}/latest.json", srv.url())));
    let o = run(leon.cmd(&rel, &srv).arg("self-update"), 30);
    assert_refused(&o, &["redirect"]);
    assert!(srv.asked().len() <= 10, "it gave up after a few: {}", srv.asked().len());
}

#[test]
fn https_is_not_downgraded_for_a_server_that_speaks_plain_http() {
    // the loopback server has no TLS: an https address must fail, not fall back
    let rel = Release::new();
    let (srv, leon) = (rel.serve(), Install::cli());
    let before = leon.state();
    let url = format!("https://127.0.0.1:{}", srv.port);
    let o = run(leon.cmd_with(&url, &Install::seam(&rel)).arg("self-update"), 60);
    assert_eq!(o.code, 1, "no TLS, no update: {}", o.text());
    assert_eq!(o.err.lines().count(), 1, "one line on stderr:\n{}", o.err);
    leon.assert_unchanged(before, "an https address without TLS");
}
