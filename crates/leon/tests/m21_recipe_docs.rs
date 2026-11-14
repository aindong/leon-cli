//! Execute every shell/config block in the recipe documentation in an isolated git project.
mod help_walk;
mod support;

use std::os::unix::fs::{symlink, PermissionsExt};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[test]
fn recipe_documentation_runs() {
    let env = support::Env::new();
    let root = help_walk::repo();
    let project = env.dir.path().join("doc-project");
    let examples = project.join("examples");
    let bin = env.dir.path().join("bin");
    std::fs::create_dir_all(&examples).unwrap();
    std::fs::create_dir(&bin).unwrap();
    symlink(env!("CARGO_BIN_EXE_leon"), bin.join("leon")).unwrap();
    for name in ["project.lua", "projects.lua"] {
        std::fs::copy(root.join("examples").join(name), examples.join(name)).unwrap();
    }
    let out = Command::new("git").args(["init", "-q"]).arg(&project).output().unwrap();
    assert!(out.status.success());
    let doc = std::fs::read_to_string(root.join("docs/recipes.md")).unwrap();
    let shell = help_walk::blocks(&doc, "sh");
    let lua = help_walk::blocks(&doc, "lua");
    assert_eq!((shell.len(), lua.len()), (3, 1));
    for (i, block) in shell.iter().enumerate() {
        let log = env.dir.path().join(format!("doc-{i}.log"));
        let mut child = Command::new("/bin/sh")
            .args(["-eu", "-c", block])
            .current_dir(&project)
            .env("PATH", format!("{}:{}", bin.display(), std::env::var("PATH").unwrap()))
            .env("LEON_RUNTIME_DIR", env.dir.path())
            .env("LEON_STATE_DIR", env.state_dir())
            .env("LEON_CONFIG", env.dir.path().join("none.lua"))
            .env("SHELL", "/bin/sh")
            .env_remove("LEON_SERVER")
            .env_remove("LEON_SESSION")
            .env_remove("LEON_BLOCK")
            .env_remove("LEON_AUTOSTART")
            .stdout(Stdio::null())
            .stderr(std::fs::File::create(&log).unwrap())
            .spawn()
            .unwrap();
        let end = Instant::now() + Duration::from_secs(30);
        let status = loop {
            if let Some(s) = child.try_wait().unwrap() {
                break s;
            }
            if Instant::now() > end {
                let _ = child.kill();
                let _ = child.wait();
                panic!("shell doc timed out");
            }
            std::thread::sleep(Duration::from_millis(20));
        };
        assert!(status.success(), "{}", std::fs::read_to_string(log).unwrap());
    }
    for code in lua {
        let config = examples.join("init.lua");
        std::fs::write(&config, code).unwrap();
        env.ok(&["config", "check", config.to_str().unwrap()]);
    }
    assert_eq!(env.json(&["ls", "--json"]).as_array().unwrap().len(), 1);
    assert_eq!(std::fs::metadata(env.dir.path()).unwrap().permissions().mode() & 0o777, 0o700);
}
