//! M22a criterion 5 (.agent/plans/2026-11-15-m22a-themes-settings.md): `leon config get/set` edit
//! init.lua in place, keep every other byte, and refuse what they can't rewrite safely.

mod support;

use std::path::{Path, PathBuf};
use std::process::Output;

use support::Env;

const INIT: &str = "-- my leon config\n\
leon.bind(\"cmd+t\", \"client.tab.new\") -- tabs\n\
\n\
-- bigger text, please\n\
leon.set( 'font.size' , 14 ) -- was 12\n\
leon.set(\"theme\", {\n  dark = \"leon-dark\", -- night\n  light = \"leon-light\",\n})\n\
local fam = \"Iosevka\"\n\
leon.set(\"font.family\", fam)\n\
for _, k in ipairs({ \"done\" }) do leon.set(\"notifications.\" .. k, true) end\n";

fn setup(src: &str) -> (Env, PathBuf) {
    let env = Env::new();
    let path = env.dir.path().join("init.lua");
    std::fs::write(&path, src).unwrap();
    (env, path)
}

fn config(env: &Env, path: &Path, args: &[&str]) -> Output {
    env.cmd().env("LEON_CONFIG", path).arg("config").args(args).output().unwrap()
}

fn text(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned() + &String::from_utf8_lossy(&o.stderr)
}

fn get(env: &Env, path: &Path, key: &str) -> String {
    let o = config(env, path, &["get", key]);
    assert!(o.status.success(), "get {key}: {}", text(&o));
    String::from_utf8_lossy(&o.stdout).trim().to_string()
}

#[test]
fn set_rewrites_only_the_value_and_keeps_comments() {
    let (env, path) = setup(INIT);
    assert_eq!(get(&env, &path, "font.size"), "14");
    let o = config(&env, &path, &["set", "font.size", "15"]);
    assert!(o.status.success(), "{}", text(&o));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), INIT.replace("'font.size' , 14 )", "'font.size' , 15 )"));
    assert_eq!(get(&env, &path, "font.size"), "15");

    let o = config(&env, &path, &["set", "theme", "dark=leon-light,light=leon-dark"]);
    assert!(o.status.success(), "{}", text(&o));
    let now = std::fs::read_to_string(&path).unwrap();
    assert!(now.contains("-- my leon config\n") && now.contains("-- was 12\n"), "{now}");
    assert_eq!(get(&env, &path, "theme"), "dark=leon-light,light=leon-dark");

    let o = config(&env, &path, &["set", "theme", "leon-light"]);
    assert!(o.status.success(), "{}", text(&o));
    assert_eq!(get(&env, &path, "theme"), "leon-light");
}

#[test]
fn a_key_not_set_yet_is_appended() {
    let (env, path) = setup("-- empty but mine\n");
    let o = config(&env, &path, &["set", "copy_on_select", "true"]);
    assert!(o.status.success(), "{}", text(&o));
    let now = std::fs::read_to_string(&path).unwrap();
    assert!(now.starts_with("-- empty but mine\n"), "{now}");
    assert!(now.contains("leon.set(\"copy_on_select\", true)"), "{now}");
    assert_eq!(get(&env, &path, "copy_on_select"), "true");

    // no file yet: set creates it
    let fresh = env.dir.path().join("new/init.lua");
    let o = config(&env, &fresh, &["set", "font.family", "JetBrains Mono"]);
    assert!(o.status.success(), "{}", text(&o));
    assert_eq!(get(&env, &fresh, "font.family"), "JetBrains Mono");
}

#[test]
fn refuses_what_it_cannot_rewrite_and_leaves_the_file() {
    let (env, path) = setup(INIT);
    for (args, want) in [
        (&["set", "font.family", "Menlo"][..], "init.lua:11"),
        (&["set", "notifications.done", "false"][..], "init.lua:12"),
        (&["set", "font.size", "99"][..], "8"),
        (&["set", "font.size", "big"][..], "font.size"),
        (&["set", "theme", "no-such-theme"][..], "no-such-theme"),
        (&["set", "colour", "red"][..], "unknown setting"),
    ] {
        let o = config(&env, &path, args);
        assert_eq!(o.status.code(), Some(1), "{args:?}: {}", text(&o));
        assert!(text(&o).contains(want), "{args:?}: {} should mention {want:?}", text(&o));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), INIT, "{args:?} left the file alone");
    }
    let o = config(&env, &path, &["get", "colour"]);
    assert_eq!(o.status.code(), Some(1), "{}", text(&o));
}

#[test]
fn a_symlinked_config_stays_a_symlink() {
    let (env, real) = setup("leon.set(\"font.size\", 12)\n");
    let link = env.dir.path().join("link.lua");
    std::os::unix::fs::symlink(&real, &link).unwrap();
    let o = config(&env, &link, &["set", "font.size", "16"]);
    assert!(o.status.success(), "{}", text(&o));
    assert!(std::fs::symlink_metadata(&link).unwrap().file_type().is_symlink());
    assert_eq!(std::fs::read_to_string(&real).unwrap(), "leon.set(\"font.size\", 16)\n");
}

#[test]
fn a_list_setting_round_trips() {
    let (env, path) = setup("-- hints\nleon.set(\"hint_patterns\", { \"OLD-\\\\d+\" }) -- tickets\n");
    assert_eq!(get(&env, &path, "hint_patterns"), r#"["OLD-\\d+"]"#);
    let o = config(&env, &path, &["set", "hint_patterns", r#"["TICKET-\\d+", "id=(\\w+)"]"#]);
    assert!(o.status.success(), "{}", text(&o));
    let now = std::fs::read_to_string(&path).unwrap();
    assert!(now.starts_with("-- hints\n") && now.ends_with(" -- tickets\n"), "{now}");
    assert_eq!(get(&env, &path, "hint_patterns"), r#"["TICKET-\\d+","id=(\\w+)"]"#);
    let o = config(&env, &path, &["set", "hint_patterns", "TICKET"]);
    assert_eq!(o.status.code(), Some(1), "a bare word is not a list: {}", text(&o));
}
