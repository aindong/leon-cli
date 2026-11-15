//! Release audit: the server holds `server.lock` for its life, so a second server starting beside
//! it now waits on that lock. Commands that autostart at once must all still reach one server, and
//! a start right after a stop must still come up.

mod support;

use support::Env;

#[test]
fn eight_autostarts_at_once_all_reach_a_server() {
    let env = Env::new();
    let children: Vec<_> =
        (0..8).map(|_| env.cmd().args(["ls"]).stdout(std::process::Stdio::null()).spawn().unwrap()).collect();
    for mut c in children {
        let st = c.wait().unwrap();
        assert!(st.success(), "an autostarting `leon ls` failed: {st:?}");
    }
    env.ok(&["server", "status"]);
}

#[test]
fn a_start_right_after_a_stop_comes_up() {
    let env = Env::new();
    for _ in 0..3 {
        env.ok(&["server", "start"]);
        env.ok(&["run", "--", "sleep", "300"]);
        env.ok(&["server", "stop"]);
    }
    env.ok(&["server", "start"]);
    env.ok(&["server", "status"]);
}
