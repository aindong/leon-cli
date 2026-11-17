//! Settings › Server, the CLI side: a server `leon` starts in the background reports the log
//! file its output goes to, so the page can reveal it; the docs name the two new sections.

mod support;

use support::Env;

#[test]
fn a_background_server_reports_its_log() {
    let env = Env::new();
    env.ok(&["server", "start"]);
    let s = env.json(&["api", "call", "server.status"]);
    let log = env.socket().with_file_name("server.log");
    assert_eq!(s["log"].as_str(), Some(log.display().to_string().as_str()), "{s}");
    assert!(s["started_at"].as_u64().is_some_and(|t| t > 0) && s["blocks"] == 0, "{s}");
}

#[test]
fn the_docs_name_server_and_remote_hosts() {
    let settings = include_str!("../../../docs/lua/settings.md");
    assert!(settings.contains("Remote Hosts lists the hosts `leon hosts`"), "Remote Hosts");
    assert!(settings.contains("Server shows This Mac's server"), "Server");
    assert!(settings.contains("since every program in every session ends"), "what Stop does");
}
