//! A fake `tailscale` on PATH: it logs its argv to `argv` in its dir and prints the recorded status
//! in `status` (a file from tests/fixtures/tailscale), exiting with the number in `rc` (default 0).
//! While `hang` exists it waits on a child that keeps its stdout open after it is killed; while
//! `flood` exists it prints forever. No test talks to a real tailnet.

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

const FAKE: &str = r#"#!/bin/sh
d=$(dirname "$0")
printf '%s\n' "$*" >> "$d/argv"
if [ -e "$d/hang" ]; then sleep 30; exit 0; fi
if [ -e "$d/flood" ]; then exec yes '{"BackendState":"Running","Peer":{'; fi
cat "$d/status"
exit "$(cat "$d/rc" 2>/dev/null || echo 0)"
"#;

pub struct Fake {
    pub dir: tempfile::TempDir,
}

pub fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tailscale").join(name)
}

impl Fake {
    /// A fake printing the fixture `name` (`running.json`, …).
    pub fn new(name: &str) -> Fake {
        let dir = tempfile::tempdir().unwrap();
        let bin = dir.path().join("tailscale");
        std::fs::write(&bin, FAKE).unwrap();
        std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).unwrap();
        std::fs::copy(fixture(name), dir.path().join("status")).unwrap();
        Fake { dir }
    }

    /// PATH with the fake first.
    pub fn path(&self) -> String {
        format!("{}:{}", self.dir.path().display(), std::env::var("PATH").unwrap_or_default())
    }

    /// Run `c` with the fake found on PATH (and no `LEON_TAILSCALE` to override it).
    pub fn apply<'a>(&self, c: &'a mut Command) -> &'a mut Command {
        c.env("PATH", self.path()).env_remove("LEON_TAILSCALE")
    }

    /// Every argv the fake was run with, one per line.
    pub fn argv(&self) -> String {
        std::fs::read_to_string(self.dir.path().join("argv")).unwrap_or_default()
    }

    /// Only ever `status --json`: leon never changes Tailscale.
    pub fn only_read(&self) {
        for call in self.argv().lines() {
            assert_eq!(call, "status --json", "leon only reads the status: {}", self.argv());
        }
    }

    pub fn set(&self, file: &str, text: &str) {
        std::fs::write(self.dir.path().join(file), text).unwrap();
    }
}
