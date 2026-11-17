//! M7b criterion 7 (.agent/plans/2026-11-14-m7b-ship.md): the examples in docs/lua.md and docs/lua/ run. A fenced
//! block tagged `lua` is a config and must pass `leon config check`; one tagged `lua script` must
//! exit 0 under `leon do -e` on a private server. Neither may print a warning.

mod help_walk;
mod support;

use std::time::{Duration, Instant};

/// (info string, code) of each fenced block tagged `lua…`.
fn lua_blocks(doc: &str) -> Vec<(String, String)> {
    let mut out = vec![];
    let mut cur: Option<(String, String)> = None;
    for line in doc.lines() {
        if let Some(info) = line.trim_start().strip_prefix("```") {
            match cur.take() {
                Some(b) => out.push(b),
                None => cur = Some((info.trim().to_string(), String::new())),
            }
        } else if let Some((_, code)) = cur.as_mut() {
            code.push_str(line);
            code.push('\n');
        }
    }
    out.retain(|(info, _)| info == "lua" || info.starts_with("lua "));
    out
}

/// docs/lua.md and its pages in docs/lua/.
fn doc() -> String {
    let dir = help_walk::repo().join("docs/lua");
    let mut pages: Vec<_> = std::fs::read_dir(&dir)
        .expect("docs/lua/ exists")
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "md"))
        .collect();
    pages.sort();
    assert!(pages.len() >= 4, "docs/lua/ has the Lua pages: {pages:?}");
    pages.into_iter().fold(std::fs::read_to_string(dir.with_extension("md")).expect("docs/lua.md exists"), |all, p| {
        all + &std::fs::read_to_string(p).unwrap()
    })
}

#[test]
fn config_examples_pass_config_check() {
    let configs: Vec<String> = lua_blocks(&doc()).into_iter().filter(|(i, _)| i == "lua").map(|(_, c)| c).collect();
    assert!(configs.len() >= 3, "docs/lua.md should have at least 3 config examples, has {}", configs.len());
    let env = support::Env::new();
    for (i, code) in configs.iter().enumerate() {
        let path = env.dir.path().join(format!("example{i}.lua"));
        std::fs::write(&path, code).unwrap();
        let o = env.run(&["config", "check", path.to_str().unwrap()]);
        let out = String::from_utf8_lossy(&o.stdout).into_owned() + &String::from_utf8_lossy(&o.stderr);
        assert!(o.status.success(), "config example {i} failed `leon config check`:\n{code}\n---\n{out}");
        assert!(!out.contains("warning"), "config example {i} has warnings:\n{code}\n---\n{out}");
    }
}

#[test]
fn script_examples_run_under_leon_do() {
    let scripts: Vec<String> =
        lua_blocks(&doc()).into_iter().filter(|(i, _)| i == "lua script").map(|(_, c)| c).collect();
    assert!(scripts.len() >= 3, "docs/lua.md should have at least 3 script examples, has {}", scripts.len());
    let env = support::Env::new();
    let (s, b) = env.session();
    for (i, code) in scripts.iter().enumerate() {
        let err = env.dir.path().join(format!("script{i}.err"));
        let mut child = env
            .cmd()
            .args(["do", "-s", &s, "-b", &b, "-e", code])
            .stdout(std::process::Stdio::null())
            .stderr(std::fs::File::create(&err).unwrap())
            .spawn()
            .unwrap();
        let end = Instant::now() + Duration::from_secs(30);
        let status = loop {
            if let Some(st) = child.try_wait().unwrap() {
                break st;
            }
            if Instant::now() > end {
                let _ = child.kill();
                panic!("script example {i} still running after 30 s:\n{code}");
            }
            std::thread::sleep(Duration::from_millis(50));
        };
        let err = std::fs::read_to_string(&err).unwrap();
        assert!(status.success(), "script example {i} exited {:?}:\n{code}\n---\n{err}", status.code());
        assert!(err.trim().is_empty(), "script example {i} printed to stderr:\n{code}\n---\n{err}");
    }
}
