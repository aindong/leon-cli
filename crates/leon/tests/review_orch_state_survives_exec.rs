//! Review: what an agent reported holds while its process stays in front, even when that process
//! execs another program (its name changes, its process group doesn't).

mod support;

use std::time::Duration;

use support::orch::Orch;

#[test]
fn a_report_holds_across_an_exec() {
    let o = Orch::new();
    let (s, boss) = o.env.session();
    // in front long enough to be sampled by name, reports, then becomes `sleep`
    let agent =
        o.script("exec-agent", "sleep 1; leon notify --state done --body finished; echo reported; exec sleep 300");
    let w = o.ok_inside(&s, &boss, &["agent", "start", agent.to_str().unwrap(), "t"]);
    o.env.screen_until(&w, "the report", |t| t.contains("reported"));
    std::thread::sleep(Duration::from_secs(1)); // the server's 200 ms look sees `sleep` in front
    let team = o.json_inside(&s, &boss, &["agent", "ls", "--json"]);
    assert_eq!(team["workers"][0]["state"], "done", "the report was dropped when the program exec'd: {team}");
}
