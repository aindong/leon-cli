//! M20b criterion 12: `leon actions` lists the app's float and drawer actions (source `app`).

mod support;

use support::Env;

const ACTIONS: [&str; 7] = [
    "pane.float",
    "pane.new_floating",
    "pane.unfloat",
    "pane.scratch.toggle",
    "pane.dock.left",
    "pane.dock.right",
    "pane.dock.bottom",
];

#[test]
fn leon_actions_lists_the_float_actions() {
    let e = Env::new();
    e.ok(&["server", "start"]);
    let text = e.ok(&["actions"]);
    let json = e.json(&["actions", "--json"]);
    for name in ACTIONS {
        assert!(text.lines().any(|l| l.starts_with(name) && l.trim_end().ends_with("app")), "{name} in:\n{text}");
        let row = json.as_array().unwrap().iter().find(|a| a["name"] == name);
        assert!(row.is_some_and(|a| a["source"] == "app"), "{name} in --json: {json}");
    }
}
