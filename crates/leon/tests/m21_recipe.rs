//! Exercise the shipped recipe against real private servers and git worktrees.
mod support;

use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};
use support::Env;

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}
fn git(dir: &Path, args: &[&str]) {
    let out = Command::new("git").arg("-C").arg(dir).args(args).output().unwrap();
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
}
fn bounded(mut cmd: Command) -> Output {
    let out = tempfile::tempfile().unwrap();
    let err = tempfile::tempfile().unwrap();
    cmd.stdout(out.try_clone().unwrap()).stderr(err.try_clone().unwrap()).stdin(Stdio::null());
    let mut child = cmd.spawn().unwrap();
    let end = Instant::now() + Duration::from_secs(40);
    let status = loop {
        if let Some(s) = child.try_wait().unwrap() {
            break s;
        }
        if Instant::now() > end {
            let _ = child.kill();
            let _ = child.wait();
            panic!("recipe timed out");
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    use std::io::{Read, Seek};
    let read = |mut f: std::fs::File| {
        f.rewind().unwrap();
        let mut b = Vec::new();
        f.read_to_end(&mut b).unwrap();
        b
    };
    Output { status, stdout: read(out), stderr: read(err) }
}
fn recipe(env: &Env, dir: &Path, args: &[&str]) -> Output {
    let mut cmd = env.cmd();
    cmd.current_dir(dir).args(["do", repo().join("examples/projects.lua").to_str().unwrap()]).args(args);
    bounded(cmd)
}
fn success(out: Output) -> String {
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8(out.stdout).unwrap()
}
fn cwd(env: &Env, sid: &str) -> PathBuf {
    let view = env.window(sid, 0);
    let b = view["block_ids"][0].as_str().unwrap();
    let end = Instant::now() + Duration::from_secs(5);
    loop {
        let v = env.json(&["block", "inspect", b, "--json"]);
        if let Some(path) = v["cwd"].as_str() {
            return PathBuf::from(path).canonicalize().unwrap();
        }
        assert!(Instant::now() < end, "no cwd for {b}: {v}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn script_reuses_project_and_opens_each_worktree_with_ignored_file_hook() {
    let env = Env::new();
    let main = env.dir.path().join("main project");
    let parent = env.dir.path().join("parent space");
    std::fs::create_dir(&parent).unwrap();
    let other = parent.join("branch 'quoted'");
    std::fs::create_dir(&main).unwrap();
    git(&main, &["init", "-q"]);
    std::fs::write(main.join(".gitignore"), ".env\n").unwrap();
    git(&main, &["add", ".gitignore"]);
    git(&main, &["-c", "user.name=Test", "-c", "user.email=test@example.test", "commit", "-qm", "initial"]);
    git(&main, &["worktree", "add", "-qb", "feature", other.to_str().unwrap()]);
    std::fs::write(main.join(".env"), "PRIVATE=example\n").unwrap();
    let first = success(recipe(&env, &main, &["dir=."]));
    assert_eq!(cwd(&env, first.trim()), main.canonicalize().unwrap());
    assert_eq!(first, success(recipe(&env, &main, &["dir=."])));
    let source = format!("copy_from={}", main.display());
    let ids = success(recipe(&env, &main, &["dir=.", "worktrees=true", &source, "copy_files=[\".env\"]"]));
    assert_eq!(ids.lines().count(), 2);
    let sessions = env.json(&["ls", "--json"]);
    assert_eq!(sessions.as_array().unwrap().len(), 2);
    for s in sessions.as_array().unwrap() {
        let p = cwd(&env, s["session_id"].as_str().unwrap());
        assert!(p == main.canonicalize().unwrap() || p == other.canonicalize().unwrap());
        assert_eq!(s["label"].as_str().unwrap(), p.file_name().unwrap().to_str().unwrap());
    }
    assert_eq!(std::fs::read(other.join(".env")).unwrap(), b"PRIVATE=example\n");
    use std::os::unix::fs::PermissionsExt;
    assert_eq!(std::fs::metadata(other.join(".env")).unwrap().permissions().mode() & 0o777, 0o600);
    std::fs::write(other.join(".env"), "KEEP\n").unwrap();
    success(recipe(&env, &main, &["dir=.", "worktrees=true", &source, "copy_files=[\".env\"]"]));
    assert_eq!(std::fs::read(other.join(".env")).unwrap(), b"KEEP\n");
}

#[test]
fn config_action_is_runnable_and_reuses_the_session() {
    let env = Env::new();
    let config = repo().join("examples/project.lua");
    let out = env.cmd().env("LEON_CONFIG", config).args(["server", "start"]).output().unwrap();
    success(out);
    let arg = format!("dir={}", env.dir.path().canonicalize().unwrap().display());
    let a = env.json(&["do", "project_open", &arg]);
    let b = env.json(&["do", "project_open", &arg]);
    assert_eq!(a, b);
    assert_eq!(cwd(&env, a[0].as_str().unwrap()), env.dir.path().canonicalize().unwrap());
}

#[test]
fn copy_hook_rejects_traversal_and_tracked_files_and_preserves_existing_files() {
    let env = Env::new();
    let source = env.dir.path().join("source");
    let dest = env.dir.path().join("dest");
    std::fs::create_dir(&source).unwrap();
    std::fs::create_dir(&dest).unwrap();
    git(&source, &["init", "-q"]);
    std::fs::write(source.join(".gitignore"), ".env\n.tracked\n").unwrap();
    std::fs::write(source.join(".tracked"), "tracked secret").unwrap();
    git(&source, &["add", "-f", ".tracked"]);
    std::fs::write(source.join(".env"), "secret").unwrap();
    std::fs::write(dest.join(".env"), "existing").unwrap();
    let from = format!("copy_from={}", source.display());
    for file in ["../.env", ".gitignore", ".tracked"] {
        let files = format!("copy_files={}", serde_json::json!([file]));
        let out = recipe(&env, &dest, &["dir=.", &from, &files]);
        assert!(!out.status.success(), "must reject {file}");
        assert_eq!(env.json(&["ls", "--json"]), serde_json::json!([]));
    }
    success(recipe(&env, &dest, &["dir=.", &from, "copy_files=[\".env\"]"]));
    assert_eq!(std::fs::read(dest.join(".env")).unwrap(), b"existing");
}

#[test]
fn a_newline_in_a_parent_path_reaches_the_real_shell() {
    let env = Env::new();
    let dir = env.dir.path().join("parent\nline").join("project");
    std::fs::create_dir_all(&dir).unwrap();
    let sid = success(recipe(&env, &dir, &["dir=."]));
    let view = env.window(sid.trim(), 0);
    let block = view["block_ids"][0].as_str().unwrap();
    // cwd metadata deliberately suppresses control characters. Check the PTY itself,
    // with hex output so terminal line breaking cannot change the path we compare.
    env.ok(&["send", "-b", block, "printf 'M21_HEX='; pwd | od -An -tx1 | tr -d ' \\n'; printf '\\n'\n"]);
    use std::os::unix::ffi::OsStrExt;
    let mut bytes = dir.canonicalize().unwrap().as_os_str().as_bytes().to_vec();
    bytes.push(b'\n');
    let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    // Long lines may wrap on the terminal's default width.
    env.screen_until(block, "the complete cwd bytes", |s| s.replace('\n', "").contains(&format!("M21_HEX={hex}")));
}

#[test]
fn ignored_file_copy_refuses_source_and_destination_symlinks() {
    use std::os::unix::fs::symlink;
    let env = Env::new();
    let source = env.dir.path().join("source");
    let dest = env.dir.path().join("dest");
    std::fs::create_dir(&source).unwrap();
    std::fs::create_dir(&dest).unwrap();
    git(&source, &["init", "-q"]);
    std::fs::write(source.join(".gitignore"), ".env\n").unwrap();
    let external = env.dir.path().join("keep");
    std::fs::write(&external, "unchanged").unwrap();
    symlink(&external, source.join(".env")).unwrap();
    let from = format!("copy_from={}", source.display());
    let out = recipe(&env, &dest, &["dir=.", &from, "copy_files=[\".env\"]"]);
    assert!(!out.status.success());
    assert!(!dest.join(".env").exists());
    std::fs::remove_file(source.join(".env")).unwrap();
    std::fs::write(source.join(".env"), "new").unwrap();
    symlink(&external, dest.join(".env")).unwrap();
    let out = recipe(&env, &dest, &["dir=.", &from, "copy_files=[\".env\"]"]);
    assert!(!out.status.success());
    assert_eq!(std::fs::read(&external).unwrap(), b"unchanged");
    assert_eq!(env.json(&["ls", "--json"]), serde_json::json!([]));
}
