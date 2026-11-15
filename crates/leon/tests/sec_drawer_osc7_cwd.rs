//! Security review: the drawer must not open where an exited program's printed OSC 7 points.
mod support;

use std::path::Path;
use support::Env;

#[test]
fn drawer_ignores_an_exited_programs_osc7_path() {
    let env = Env::new();
    env.ok(&["server", "start"]);
    let planted = env.dir.path().join("planted");
    std::fs::create_dir(&planted).unwrap();
    env.ok(&["new", "work", "--cwd", "/"]);
    let p = planted.to_str().unwrap();
    let b =
        env.ok(&["-s", "work", "run", "--keep-open", "-c", "/", "--", "printf", "\\033]7;file://%s\\007done\\n", p]);
    env.screen_until(&b, "the program's output", |s| s.contains("done"));
    env.code(&["wait", &b]);
    env.ok(&["-s", "work", "toggle", "scratch", "--create"]);
    let drawer = env.json(&["-s", "work", "block", "inspect", "scratch", "--json"]);
    let cwd = drawer["cwd"].as_str().unwrap_or("/");
    let real = |x: &Path| x.canonicalize().unwrap();
    assert_ne!(real(Path::new(cwd)), real(&planted), "the drawer opened in a directory named by program output");
}
