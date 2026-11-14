//! M16a: a "remote" machine for `leon -H` tests. `LEON_SSH` points at a fake ssh that logs its
//! argv, then runs the remote command with a second env's private runtime and state dirs, a PATH
//! holding this build's `leon`, and its own HOME. A host named `unreachable…` fails as ssh would.

use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::time::{Duration, Instant};

use serde_json::Value;

use super::{stream, Env, Stream};

const FAKE_SSH: &str = r#"#!/bin/sh
printf '%s\037' "$@" >> "$FAKE_SSH_DIR/argv"
printf '\n' >> "$FAKE_SSH_DIR/argv"
echo $$ >> "$FAKE_SSH_DIR/pids"
while [ $# -gt 0 ]; do
  case $1 in
    --) shift; break ;;
    -p|-o|-l|-i|-J|-F) shift 2 ;;
    -*) shift ;;
    *) break ;;
  esac
done
host=$1
shift
case $host in
  unreachable*) echo "ssh: connect to host $host port 22: Connection refused" >&2; exit 255 ;;
esac
cd "$FAKE_HOME" || exit 255
# bash, not sh: it execs the command in place, as macOS's sh does and Debian's dash doesn't, so the
# pid logged above ends up as the remote leon and killing it drops the link like a dead ssh
exec env HOME="$FAKE_HOME" PATH="$FAKE_PATH" LEON_RUNTIME_DIR="$FAKE_RUNTIME" LEON_STATE_DIR="$FAKE_STATE" bash -c "$*"
"#;

pub struct Remote {
    /// This machine: where `leon -H` runs.
    pub local: Env,
    /// The other machine's dirs, reached only through the fake ssh.
    pub remote: Env,
    /// The fake ssh, its logs, a bin dir with `leon`, and the remote HOME.
    pub fake: tempfile::TempDir,
    /// The remote PATH [default: the bin dir with `leon`, then the system dirs].
    pub path: String,
}

impl Remote {
    pub fn new() -> Self {
        let fake = tempfile::Builder::new().permissions(std::fs::Permissions::from_mode(0o700)).tempdir().unwrap();
        let ssh = fake.path().join("ssh");
        std::fs::write(&ssh, FAKE_SSH).unwrap();
        std::fs::set_permissions(&ssh, std::fs::Permissions::from_mode(0o700)).unwrap();
        let bin = fake.path().join("bin");
        std::fs::create_dir(&bin).unwrap();
        std::os::unix::fs::symlink(env!("CARGO_BIN_EXE_leon"), bin.join("leon")).unwrap();
        std::fs::create_dir(fake.path().join("home")).unwrap();
        let path = format!("{}:/usr/bin:/bin:/usr/sbin:/sbin", bin.display());
        Remote { local: Env::new(), remote: Env::new(), fake, path }
    }

    pub fn home(&self) -> PathBuf {
        self.fake.path().join("home")
    }

    /// `leon` on this machine, with `LEON_SSH` set to the fake.
    pub fn cmd(&self) -> Command {
        let mut c = self.local.cmd();
        c.env("LEON_SSH", self.fake.path().join("ssh"))
            .env("FAKE_SSH_DIR", self.fake.path())
            .env("FAKE_HOME", self.home())
            .env("FAKE_PATH", &self.path)
            .env("FAKE_RUNTIME", self.remote.dir.path())
            .env("FAKE_STATE", self.remote.state_dir());
        c
    }

    /// The same variables, for a test's own PTY (`Host::spawn`) or process.
    pub fn vars(&self) -> Vec<(String, String)> {
        let c = self.cmd();
        c.get_envs()
            .filter(|(k, _)| k.to_str().is_some_and(|k| k == "LEON_SSH" || k.starts_with("FAKE_")))
            .map(|(k, v)| (k.to_string_lossy().into_owned(), v.unwrap().to_string_lossy().into_owned()))
            .collect()
    }

    pub fn run(&self, args: &[&str]) -> Output {
        self.cmd().args(args).output().unwrap()
    }

    pub fn ok(&self, args: &[&str]) -> String {
        let out = self.run(args);
        assert!(
            out.status.success(),
            "leon {args:?} failed ({:?}): {}",
            out.status.code(),
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8(out.stdout).unwrap().trim().to_string()
    }

    pub fn json(&self, args: &[&str]) -> Value {
        let out = self.ok(args);
        serde_json::from_str(&out).unwrap_or_else(|e| panic!("leon {args:?}: not JSON ({e}): {out}"))
    }

    pub fn code(&self, args: &[&str]) -> i32 {
        self.run(args).status.code().unwrap()
    }

    /// `leon ARGS events --json` through the fake ssh, past its snapshot line.
    pub fn events(&self, args: &[&str]) -> Stream {
        stream(self.cmd(), args)
    }

    /// Every argv the fake ssh was run with, oldest first.
    pub fn ssh_calls(&self) -> Vec<Vec<String>> {
        let text = std::fs::read_to_string(self.fake.path().join("argv")).unwrap_or_default();
        text.lines().map(|l| l.split('\x1f').filter(|a| !a.is_empty()).map(str::to_string).collect()).collect()
    }

    /// SIGKILL every transport still running: each fake ssh became `leon server proxy` by exec, so
    /// only pids whose command still says so are killed (a finished one's pid may be reused).
    /// Returns how many were killed.
    pub fn kill_transports(&self) -> usize {
        let pids = std::fs::read_to_string(self.fake.path().join("pids")).unwrap_or_default();
        let mut killed = 0;
        for pid in pids.lines().filter_map(|p| p.trim().parse::<i32>().ok()) {
            let out = Command::new("ps").args(["-o", "command=", "-p", &pid.to_string()]).output().unwrap();
            if String::from_utf8_lossy(&out.stdout).contains("server proxy") {
                // SAFETY: a signal to a process this test started
                unsafe { libc::kill(pid, libc::SIGKILL) };
                killed += 1;
            }
        }
        killed
    }

    /// Wait until `pred` holds, for up to 10 s.
    pub fn until(&self, what: &str, pred: impl Fn() -> bool) {
        let end = Instant::now() + Duration::from_secs(10);
        while !pred() {
            assert!(Instant::now() < end, "timed out waiting for {what}");
            std::thread::sleep(Duration::from_millis(50));
        }
    }
}
