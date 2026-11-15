//! A socket path over the platform's limit (sun_path: 104 bytes on macOS, 108 on Linux) fails at
//! once, saying the path, its length, the limit and the fix, from `leon server start` and from a
//! command that would start a server.

mod support;

use std::os::unix::fs::DirBuilderExt;
use std::time::{Duration, Instant};

use support::Env;

#[test]
fn a_socket_path_over_the_limit_fails_fast_and_says_why() {
    let env = Env::new();
    let limit = if cfg!(target_os = "macos") { 104 } else { 108 };
    let mut dir = env.dir.path().join("run");
    while dir.join("server.sock").as_os_str().len() <= limit {
        dir.push("a-long-directory-name");
    }
    std::fs::DirBuilder::new().recursive(true).mode(0o700).create(&dir).unwrap();
    let sock = dir.join("server.sock");
    let len = sock.as_os_str().len().to_string();
    for args in [&["server", "start"][..], &["ls"][..]] {
        let t = Instant::now();
        let out = env.cmd().env("LEON_RUNTIME_DIR", &dir).args(args).output().unwrap();
        let err = String::from_utf8_lossy(&out.stderr);
        assert!(!out.status.success(), "{args:?} failed");
        assert!(t.elapsed() < Duration::from_secs(2), "{args:?} failed fast: {:?}", t.elapsed());
        for word in [sock.to_str().unwrap(), len.as_str(), &limit.to_string(), "LEON_RUNTIME_DIR"] {
            assert!(err.contains(word), "{args:?} names {word:?}: {err}");
        }
    }
}
