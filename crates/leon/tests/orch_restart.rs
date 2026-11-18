//! Agent orchestration criterion 8: a server restart keeps who works for whom and the unread
//! reports: both are saved with the sessions.

mod support;

use support::orch::{private, Orch};

#[test]
fn the_team_and_its_unread_reports_survive_a_restart() {
    let o = Orch::new();
    let (s, boss) = o.env.session();
    let agent = o.script("idle-agent", "exec sleep 300");
    let w = o.ok_inside(&s, &boss, &["agent", "start", "--label", "kept", agent.to_str().unwrap(), "t"]);
    o.env.ok(&["notify", "-b", &w, "--state", "done", "--body", "before the restart"]);
    assert_eq!(o.json_inside(&s, &boss, &["inbox", "--json", "--peek"]).as_array().unwrap().len(), 1);

    o.env.ok(&["server", "stop"]);
    let out = private(o.env.cmd(), &o.home).args(["server", "start"]).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));

    let team = o.json_inside(&s, &boss, &["agent", "ls", "--json"]);
    let ws = team["workers"].as_array().unwrap_or_else(|| panic!("{team}"));
    assert_eq!(ws.len(), 1, "{team}");
    assert_eq!((ws[0]["block_id"].as_str(), ws[0]["label"].as_str()), (Some(w.as_str()), Some("kept")));
    assert_eq!(ws[0]["agent"], "idle-agent");
    let inbox = o.json_inside(&s, &boss, &["inbox", "--json"]);
    assert_eq!(inbox[0]["body"], "before the restart", "{inbox}");
}
