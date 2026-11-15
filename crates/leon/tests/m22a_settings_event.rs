//! M22a criterion 7 (.agent/plans/2026-11-15-m22a-themes-settings.md): a private server reports the
//! effective settings (`server.settings`) and tells `--all` subscribers when they change.

mod support;

use std::time::Duration;

use support::Env;

#[test]
fn settings_method_and_change_event() {
    let env = Env::new();
    let path = env.dir.path().join("init.lua");
    std::fs::write(&path, "-- start\nleon.set(\"font.size\", 14)\n").unwrap();
    let out = env.cmd().env("LEON_CONFIG", &path).args(["server", "start"]).output().unwrap();
    assert!(out.status.success(), "server start: {}", String::from_utf8_lossy(&out.stderr));

    let r = env.json(&["api", "call", "server.settings"]);
    let s = &r["settings"];
    assert_eq!(s["font"]["size"], 14.0, "{r}");
    assert_eq!(s["font"]["family"], "Menlo", "defaults filled in: {r}");
    assert_eq!(s["theme"], serde_json::json!({ "dark": "leon-dark", "light": "leon-dark" }), "{r}");
    assert_eq!(s["notifications"]["blocked"], true, "{r}");
    assert_eq!(s["copy_on_select"], false, "{r}");
    assert_eq!((s["repeat_time"].as_u64(), &s["hint_patterns"]), (Some(500), &serde_json::json!([])), "{r}");
    assert!(r["sources"]["font.size"].as_str().unwrap().ends_with("init.lua:2"), "{r}");

    let mut events = env.events(&["--all"]);
    let o = env.cmd().env("LEON_CONFIG", &path).args(["config", "set", "theme", "leon-light"]).output().unwrap();
    assert!(o.status.success(), "config set: {}", String::from_utf8_lossy(&o.stderr));
    // the set's reload sends settings_changed and keymap_changed (generation 2), in either order
    let (mut settings, mut keymap) = (None, false);
    while settings.is_none() || !keymap {
        let e = events.next();
        match e.name.as_str() {
            "settings_changed" => settings = Some(e.payload),
            "keymap_changed" => keymap = e.payload["generation"] == 2,
            _ => {}
        }
    }
    assert_eq!(settings.unwrap()["settings"]["theme"]["light"], "leon-light");
    assert_eq!(env.json(&["api", "call", "server.settings"])["settings"]["theme"]["dark"], "leon-light");

    // a reload that changes no setting says nothing about settings
    std::fs::write(&path, std::fs::read_to_string(&path).unwrap() + "leon.bind(\"ctrl+q\", \"client.tab.new\")\n")
        .unwrap();
    assert!(env.run(&["config", "reload"]).status.success());
    loop {
        let e = events.next();
        assert_ne!(e.name, "settings_changed", "an unchanged reload emitted {:?}", e);
        if e.name == "keymap_changed" && e.payload["generation"] == 3 {
            break;
        }
    }
    while let Some(line) = events.line(Duration::from_millis(500)) {
        assert!(!line.contains("settings_changed"), "an unchanged reload emitted {line}");
    }
}
