//! M22a criterion 4 (.agent/plans/2026-11-15-m22a-themes-settings.md): `leon theme list`, `import`
//! and `show` against a private config dir (`LEON_CONFIG`); no server is needed.

mod support;

use std::process::Output;

use support::Env;

const HARBOR: &str = "# Harbor: a test theme\nbackground = #14181c\nforeground = #c8ccd4\npalette = 1=#e06c75\n";

fn theme(env: &Env, args: &[&str]) -> Output {
    env.cmd().env("LEON_CONFIG", env.dir.path().join("conf/init.lua")).arg("theme").args(args).output().unwrap()
}

fn text(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned() + &String::from_utf8_lossy(&o.stderr)
}

#[test]
fn import_list_and_show() {
    let env = Env::new();
    let file = env.dir.path().join("Harbor");
    std::fs::write(&file, HARBOR).unwrap();

    let o = theme(&env, &["list"]);
    assert!(o.status.success(), "{}", text(&o));
    let out = String::from_utf8_lossy(&o.stdout);
    assert!(out.contains("leon-dark") && out.contains("leon-light"), "{out}");

    let o = theme(&env, &["import", file.to_str().unwrap(), "--name", "harbor-night"]);
    assert!(o.status.success(), "{}", text(&o));
    let stored = env.dir.path().join("conf/themes/harbor-night.theme");
    assert!(stored.is_file(), "stored under the config dir: {}", text(&o));

    let o = theme(&env, &["list"]);
    assert!(String::from_utf8_lossy(&o.stdout).contains("harbor-night"), "{}", text(&o));

    let o = theme(&env, &["show", "harbor-night"]);
    assert!(o.status.success(), "{}", text(&o));
    let out = String::from_utf8_lossy(&o.stdout).to_lowercase();
    for want in ["background = #14181c", "palette = 1=#e06c75", "chrome = #", "working = #"] {
        assert!(out.contains(want), "show prints {want}: {out}");
    }
    let o = theme(&env, &["show", "leon-light", "--json"]);
    let v: serde_json::Value = serde_json::from_slice(&o.stdout).unwrap_or_else(|e| panic!("{e}: {}", text(&o)));
    assert_eq!(v["background"], "#f5f0e7", "{v}");

    // the file stem names it when --name is left out
    let o = theme(&env, &["import", file.to_str().unwrap()]);
    assert!(o.status.success(), "{}", text(&o));
    assert!(env.dir.path().join("conf/themes/Harbor.theme").is_file());
}

#[test]
fn refusals_leave_the_store_as_it_was() {
    let env = Env::new();
    let file = env.dir.path().join("Harbor");
    std::fs::write(&file, HARBOR).unwrap();
    let f = file.to_str().unwrap();
    assert!(theme(&env, &["import", f, "--name", "one"]).status.success());
    let stored = env.dir.path().join("conf/themes/one.theme");
    let before = std::fs::read(&stored).unwrap();

    std::fs::write(&file, "background = #ffffff\nforeground = #000000\n").unwrap();
    let o = theme(&env, &["import", f, "--name", "one"]);
    assert_eq!(o.status.code(), Some(1), "{}", text(&o));
    assert!(text(&o).contains("one.theme"), "says which file is in the way: {}", text(&o));
    assert_eq!(std::fs::read(&stored).unwrap(), before, "an existing theme is never overwritten");

    for name in ["../escape", "a/b", "leon-dark", "", ".hidden"] {
        let o = theme(&env, &["import", f, "--name", name]);
        assert_eq!(o.status.code(), Some(1), "--name {name:?}: {}", text(&o));
    }
    assert!(!env.dir.path().join("conf/escape.theme").exists());

    std::fs::write(&file, "background = #zzzzzz\n").unwrap();
    let o = theme(&env, &["import", f, "--name", "bad"]);
    assert_eq!(o.status.code(), Some(1), "{}", text(&o));
    assert!(text(&o).contains("line 1"), "the parser's message: {}", text(&o));
    assert!(!env.dir.path().join("conf/themes/bad.theme").exists());

    let o = theme(&env, &["show", "missing"]);
    assert_eq!(o.status.code(), Some(1), "{}", text(&o));
    assert!(text(&o).contains("leon theme list"), "{}", text(&o));
}
