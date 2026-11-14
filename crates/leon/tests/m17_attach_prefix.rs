//! M17 criteria 7 and 8: the prefix comes from `--prefix`, else the config's
//! `leon.mode("attach", { prefix = … })`; `attach/KEY` bindings change what follows it; other
//! config bindings run while attached instead of reaching the pane.

mod support;

use support::host::{assert_restored, attach_then_stty, has_words, Host};
use support::Env;

fn one_session(env: &Env) -> String {
    env.ok(&["run", "--", "sh"]);
    env.json(&["ls", "--json"])[0]["session_id"].as_str().unwrap().to_string()
}

fn with_config(env: &Env, lua: &str) -> String {
    let path = env.dir.path().join("attach-init.lua");
    std::fs::write(&path, lua).unwrap();
    path.display().to_string()
}

#[test]
fn prefix_flag_replaces_ctrl_b() {
    let env = Env::new();
    let session = one_session(&env);
    let h = Host::spawn(&env, &attach_then_stty(&format!("--prefix ctrl+a {session}")), &[]);
    h.wait_for("attached", |_| h.alt_screen() && h.last_row().contains("leon"));
    // ctrl+b is an ordinary key now: `od` in the pane gets it
    h.send(b"od -An -tx1\r");
    h.send(b"\x02\r\x04");
    h.wait_for("od shows the ctrl+b byte", |s| has_words(s, &["02", "0a"]));
    h.send(b"\x01d");
    assert_restored(&h, 0);
}

#[test]
fn config_sets_the_prefix_and_attach_bindings() {
    let env = Env::new();
    let config = with_config(&env, "leon.mode(\"attach\", { prefix = \"ctrl+a\" })\n");
    // the server reads LEON_CONFIG when it starts, so start it with the config
    let mut start = env.cmd();
    start.env("LEON_CONFIG", &config).args(["server", "start"]);
    assert!(start.status().unwrap().success());
    let session = one_session(&env);
    let h = Host::spawn(&env, &attach_then_stty(&session), &[]);
    h.wait_for("attached", |_| h.alt_screen() && h.last_row().contains("leon"));
    h.send(b"\x01d");
    assert_restored(&h, 0);

    let env = Env::new();
    let config = with_config(&env, "leon.bind(\"attach/x\", \"client.detach\")\n");
    let mut start = env.cmd();
    start.env("LEON_CONFIG", &config).args(["server", "start"]);
    assert!(start.status().unwrap().success());
    let session = one_session(&env);
    let h = Host::spawn(&env, &attach_then_stty(&session), &[]);
    h.wait_for("attached", |_| h.alt_screen() && h.last_row().contains("leon"));
    h.send(b"\x02x");
    assert_restored(&h, 0);
}

#[test]
fn config_bindings_run_while_attached() {
    let env = Env::new();
    let lua = r#"
leon.action{ name = "tag_it", title = "Tag", run = function(ctx)
  return leon.session.set_label{ label = "tagged" }
end }
leon.bind("ctrl+g", "tag_it")
leon.action{ name = "seq_it", title = "Seq", run = function(ctx)
  return leon.session.set_label{ label = "from-sequence" }
end }
leon.bind("ctrl+b>c", "seq_it")
"#;
    let config = with_config(&env, lua);
    let mut start = env.cmd();
    start.env("LEON_CONFIG", &config).args(["server", "start"]);
    assert!(start.status().unwrap().success());
    let session = one_session(&env);
    let h = Host::spawn(&env, &format!("exec \"$LEON\" attach {session}"), &[]);
    h.wait_for("attached", |_| h.alt_screen() && h.last_row().contains("leon"));
    h.send(b"od -An -tx1\r");
    h.send(b"\x07x\r\x04"); // ctrl+g, then x
    Host::until("the action renamed the session", || env.view(&session)["label"] == "tagged");
    let s = h.wait_for("od's output", |s| has_words(s, &["78", "0a"]));
    assert!(!has_words(&s, &["07"]), "ctrl+g must not reach the pane:\n{s}");

    // the config's own `ctrl+b>c` wins over the built-in prefix c (new tab)
    h.send(b"\x02c");
    Host::until("the sequence's action ran", || env.view(&session)["label"] == "from-sequence");
    assert_eq!(env.view(&session)["windows"].as_array().unwrap().len(), 1, "no new tab");
}
