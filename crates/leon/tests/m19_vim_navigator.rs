//! M19 criterion 6 (.agent/plans/2026-11-15-m19-tmux-migrants.md): docs/lua.md's vim-navigator
//! recipe works. ctrl+h moves to the pane on the left unless vim is the pane's foreground program,
//! which gets the key instead; vim's side, `leon focus left` run from the pane, moves leon's focus.

mod help_walk;
mod support;

use serde_json::json;
use support::Env;

/// The recipe: the `lua` block of docs/lua.md that mentions nvim.
fn recipe() -> String {
    let doc = std::fs::read_to_string(help_walk::repo().join("docs/lua.md")).unwrap();
    let blocks: Vec<&str> = doc.split("```").collect();
    let lua = blocks.iter().skip(1).step_by(2).find(|b| b.starts_with("lua\n") && b.contains("nvim"));
    lua.expect("docs/lua.md has the vim-navigator recipe")["lua\n".len()..].to_string()
}

fn focus(env: &Env, sid: &str) -> String {
    env.window(sid, 0)["focused_block_id"].as_str().unwrap().to_string()
}

fn ctrl_h(env: &Env, sid: &str, block: &str) -> serde_json::Value {
    let p = json!({ "session_id": sid, "block_id": block, "key": "ctrl+h" });
    env.json(&["api", "call", "client.key", &p.to_string()])
}

#[test]
fn ctrl_h_moves_focus_or_goes_to_vim() {
    let env = Env::new();
    let config = env.dir.path().join("init.lua");
    std::fs::write(&config, recipe()).unwrap();
    let out = env.cmd().env("LEON_CONFIG", &config).args(["server", "start"]).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let check = env.run(&["config", "check", config.to_str().unwrap()]);
    assert!(check.status.success(), "{}", String::from_utf8_lossy(&check.stdout));

    let a = env.ok(&["run", "--", "sleep", "300"]);
    let sid = env.json(&["ls", "--json"])[0]["session_id"].as_str().unwrap().to_string();
    let b = env.ok(&["split", "-b", &a, "--", "sleep", "300"]);
    assert_eq!(focus(&env, &sid), b);
    let r = ctrl_h(&env, &sid, &b);
    assert_eq!(r["handled"], true, "{r}");
    assert_eq!(focus(&env, &sid), a, "a pane with no vim: ctrl+h went left");

    // a program run as vim: sleep with that argv[0] (macOS won't run a copy of /bin/sleep)
    for name in ["vim", "nvim"] {
        let v = env.ok(&["split", "-b", &a, "--", "bash", "-c", &format!("exec -a {name} sleep 300")]);
        assert_eq!(focus(&env, &sid), v);
        let r = ctrl_h(&env, &sid, &v);
        assert_eq!(r["handled"], false, "{name} in front: the key goes to it: {r}");
        assert_eq!(focus(&env, &sid), v, "and leon's focus stays");

        // vim at its left edge runs `leon focus left`, inside the pane's environment
        let out = env.cmd().env("LEON_SESSION", &sid).env("LEON_BLOCK", &v).args(["focus", "left"]).output().unwrap();
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        assert_eq!(focus(&env, &sid), a, "vim's side moved leon's focus left");
        env.ok(&["focus", &v]);
    }
}
