//! Update criterion 12 (.agent/plans/2026-11-16-self-update-core.md): a check is one plain
//! request. It carries a fixed `User-Agent` of `leon/<version>` and nothing about the machine or
//! the user, and no command but `leon self-update` talks to the network.

mod release_kit;
mod update_kit;

use update_kit::{name, run, Install, Release, VERSION};

/// A request head's header names in lower case, and its `User-Agent`.
fn headers(head: &str) -> (Vec<String>, String) {
    let lines: Vec<&str> = head.lines().skip(1).filter(|l| !l.is_empty()).collect();
    let names = lines.iter().map(|l| l.split(':').next().unwrap().to_lowercase()).collect();
    let agent = lines.iter().find_map(|l| l.to_lowercase().starts_with("user-agent:").then(|| l[11..].trim()));
    (names, agent.unwrap_or_default().to_string())
}

#[test]
fn a_request_carries_the_host_a_fixed_user_agent_and_nothing_else() {
    let rel = Release::new();
    let (srv, leon) = (rel.serve(), Install::cli());
    assert_eq!(leon.update(&rel, &srv, &["--check"]).code, 10);
    assert_eq!(srv.asked(), ["latest.json", "latest.json.minisig"], "a check asks for the manifest and its signature");
    assert_eq!(leon.update(&rel, &srv, &[]).code, 0);
    assert!(srv.asked().contains(&name("cli")), "an update also asks for the file: {:?}", srv.asked());
    let (user, host) = (std::env::var("USER").unwrap_or_default(), release_kit::sh(&leon.home, "hostname"));
    for head in srv.requests() {
        assert!(head.starts_with("GET /"), "a plain GET: {head}");
        let (mut names, agent) = headers(&head);
        names.sort();
        assert_eq!(names, ["host", "user-agent"], "only what a request must have: {head}");
        // the real version, not the one the test seam claims: a fixed string per build
        assert_eq!(agent, format!("leon/{VERSION}"), "{head}");
        for private in [user.as_str(), host.trim(), leon.home.to_str().unwrap()] {
            assert!(private.len() < 3 || !head.contains(private), "the request holds {private:?}: {head}");
        }
        assert!(!head.contains('?'), "no query string: {head}");
    }
}

#[test]
fn no_other_command_talks_to_the_network() {
    let rel = Release::new();
    let (srv, leon) = (rel.serve(), Install::cli());
    for args in [&["--version"][..], &["--help"], &["ls"], &["server", "status"], &["self-update", "--help"]] {
        run(leon.cmd(&rel, &srv).args(args), 30);
    }
    // a server and a session, then the commands that use them
    let o = run(leon.cmd(&rel, &srv).env("LEON_AUTOSTART", "1").env("SHELL", "/bin/sh").args(["new", "quiet"]), 60);
    assert_eq!(o.code, 0, "a server starts: {}", o.text());
    for args in [&["ls"][..], &["run", "--", "true"], &["server", "status"], &["server", "stop"]] {
        run(leon.cmd(&rel, &srv).args(args), 60);
    }
    assert_eq!(srv.requests(), Vec::<String>::new(), "only `leon self-update` asks the release server anything");
}
