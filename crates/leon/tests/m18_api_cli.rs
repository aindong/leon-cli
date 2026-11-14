//! M18 criterion 4 (.agent/plans/2026-11-15-m18-integrator-promise.md): `leon api list` and
//! `leon api describe` show each method's stability, and describe prints the examples.

mod support;

use support::Env;

#[test]
fn api_list_shows_stability() {
    let env = Env::new();
    let list = env.ok(&["api", "list"]);
    let line = |name: &str| -> Vec<String> {
        let l = list.lines().find(|l| l.split_whitespace().next() == Some(name));
        l.unwrap_or_else(|| panic!("no line for {name}:\n{list}")).split_whitespace().map(String::from).collect()
    };
    assert_eq!(line("session.create")[1], "stable");
    assert_eq!(line("block.watch")[1], "experimental");
    assert_eq!(line("client.keymap")[1], "internal");
    assert_eq!(line("dev.leon.terminal.write")[1], "stable", "block methods are listed too");
    for l in list.lines() {
        let word = l.split_whitespace().nth(1).unwrap_or("");
        assert!(["stable", "experimental", "internal"].contains(&word), "no stability in: {l}");
    }
    let json = env.json(&["api", "list", "--json"]);
    for m in json.as_array().unwrap() {
        assert!(m["stability"].is_string(), "{m} has stability");
    }
}

#[test]
fn api_describe_shows_stability_and_examples() {
    let env = Env::new();
    let out = env.ok(&["api", "describe", "session.create"]);
    let first = out.lines().next().unwrap();
    assert!(first.contains("session.create") && first.contains("stable"), "first line: {first}");
    assert!(out.contains("request example:") && out.contains("response example:"), "{out}");
    let json = env.json(&["api", "describe", "session.create", "--json"]);
    assert_eq!(json["stability"], "stable");
    assert!(json["request_example"].is_object());
    let block = env.ok(&["api", "describe", "dev.leon.terminal.write"]);
    assert!(block.lines().next().unwrap().contains("stable"), "{block}");
}
