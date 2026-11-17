mod support;
use support::host::{assert_restored, attach_then_stty, Host};
use support::Env;

#[test]
fn scratch_shortcut_reopens_a_hidden_float_only_window() {
    let e = Env::new();
    let docs = include_str!("../../../docs/lua/keys.md");
    let recipe = docs
        .split("<!-- m20-scratch -->")
        .nth(1)
        .unwrap()
        .split("```lua\n")
        .nth(1)
        .unwrap()
        .split("```")
        .next()
        .unwrap();
    std::fs::write(e.dir.path().join("no-init.lua"), recipe).unwrap();
    let (sid, block) = e.session();
    e.ok(&["session", "rename", &sid, "m20blank"]);
    e.ok(&["float", "-b", &block, "--label", "scratch"]);
    assert!(e.window(&sid, 0)["layout"].is_null());
    let h = Host::spawn(&e, &attach_then_stty("m20blank"), &[]);
    h.wait_text("┌");
    h.send(b"\x07");
    h.wait_for("hidden float frame", |screen| !screen.contains('┌'));
    assert_eq!(e.window(&sid, 0)["floating"][0]["hidden"], true);
    assert!(e.window(&sid, 0)["focused_block_id"].is_null());
    h.send(b"\x07");
    Host::until("scratch shortcut restores hidden float", || e.window(&sid, 0)["floating"][0]["hidden"] == false);
    h.wait_text("┌");
    h.send(b"\x02d");
    assert_restored(&h, 0);
}

#[test]
fn function_shortcut_and_new_tab_work_without_focus() {
    let e = Env::new();
    std::fs::write(
        e.dir.path().join("no-init.lua"),
        r#"
leon.bind("ctrl+g", function() assert(leon.toggle{label="scratch"}); return true end)
leon.bind("ctrl+f", "pane.float")
leon.bind("ctrl+n", "pane.new_floating")
"#,
    )
    .unwrap();
    let (sid, block) = e.session();
    e.ok(&["float", "-b", &block, "--label", "scratch"]);
    e.ok(&["-s", &sid, "toggle", "scratch"]);
    let h = Host::spawn(&e, &attach_then_stty(&sid), &[]);
    h.wait_text("empty");
    h.send(b"\x07");
    h.wait_text("┌");
    h.send(b"\x07");
    h.wait_for("hidden again", |s| !s.contains('┌'));
    h.send(b"\x02c");
    Host::until("new tab from hidden window", || e.view(&sid)["windows"].as_array().unwrap().len() == 2);
    h.send(b"\x06");
    Host::until("pane.float action", || e.window(&sid, 1)["floating"].as_array().unwrap().len() == 1);
    h.send(b"\x0e");
    Host::until("pane.new_floating action", || e.window(&sid, 1)["floating"].as_array().unwrap().len() == 2);
    h.send(b"\x02d");
    assert_restored(&h, 0);
}
