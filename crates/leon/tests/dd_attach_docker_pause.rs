//! Daily-driver F12 (.agent/plans/2026-11-15-cli-docs-polish.md, criterion 7), the research run's
//! own repro: `leon -H … attach` over system ssh to an sshd in a Linux container, `docker pause`,
//! `echo during-pause⏎` typed, unpause. Before the fix the remote prompt kept `ec`. Skips (and
//! says why) without Docker, a Linux build (`LEON_LINUX_BIN`, else target/<arch>-unknown-linux-*/release/leon,
//! else scripts/package-linux.sh's target/linux/<arch>/leon) or an image.

mod support;

use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, Instant};

use support::host::{assert_restored, attach_then_stty, Host};
use support::Env;

fn docker(args: &[&str]) -> Option<String> {
    let out = Command::new("docker").args(args).output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn linux_bin(arch: &str) -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("LEON_LINUX_BIN") {
        return Some(PathBuf::from(p)).filter(|p| p.exists());
    }
    // as m16a_docker_sshd finds it: a cross build, else scripts/package-linux.sh's binary
    let target = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target");
    let deb_arch = if matches!(arch, "aarch64" | "arm64") { "arm64" } else { "amd64" };
    let arch = if arch == "arm64" { "aarch64" } else { arch };
    ["gnu", "musl"]
        .iter()
        .map(|libc| target.join(format!("{arch}-unknown-linux-{libc}/release/leon")))
        .chain([target.join(format!("linux/{deb_arch}/leon"))])
        .find(|p| p.exists())
}

/// `docker ARGS NAME` when dropped.
struct Cleanup(&'static [&'static str], String);

impl Drop for Cleanup {
    fn drop(&mut self) {
        let _ = Command::new("docker").args(self.0).arg(&self.1).output();
    }
}

#[test]
fn keys_typed_while_the_devbox_is_paused_never_arrive_in_part() {
    let Some(arch) = docker(&["info", "--format", "{{.Architecture}}"]) else {
        eprintln!("skipped: no Docker");
        return;
    };
    let Some(bin) = linux_bin(&arch) else {
        eprintln!("skipped: no Linux leon for {arch} (run scripts/package-linux.sh, or set LEON_LINUX_BIN)");
        return;
    };
    let env = Env::new();
    let ctx = env.dir.path().join("image");
    std::fs::create_dir(&ctx).unwrap();
    std::fs::copy(&bin, ctx.join("leon")).unwrap();
    let key = env.dir.path().join("key");
    let made = Command::new("ssh-keygen").args(["-q", "-t", "ed25519", "-N", "", "-f"]).arg(&key).status().unwrap();
    assert!(made.success());
    std::fs::copy(key.with_extension("pub"), ctx.join("authorized_keys")).unwrap();
    std::fs::write(
        ctx.join("Dockerfile"),
        "FROM debian:stable-slim\n\
         RUN apt-get update && apt-get install -y --no-install-recommends openssh-server && mkdir -p /run/sshd /root/.ssh\n\
         COPY authorized_keys /root/.ssh/authorized_keys\n\
         COPY leon /usr/local/bin/leon\n\
         RUN chmod 700 /root/.ssh && chmod 600 /root/.ssh/authorized_keys && chmod 755 /usr/local/bin/leon\n\
         CMD [\"/usr/sbin/sshd\", \"-D\", \"-e\"]\n",
    )
    .unwrap();
    let tag = format!("leon-dd-pause-{}", std::process::id());
    let _image = Cleanup(&["rmi", "-f"], tag.clone());
    if docker(&["build", "-q", "-t", &tag, ctx.to_str().unwrap()]).is_none() {
        eprintln!("skipped: the sshd image didn't build (offline?)");
        return;
    }
    let id = docker(&["run", "-d", "--rm", "-p", "127.0.0.1::22", &tag]).expect("docker run");
    let _container = Cleanup(&["rm", "-f"], id.clone());
    let port = docker(&["port", &id, "22"]).expect("a published port");
    let port = port.lines().next().unwrap().rsplit(':').next().unwrap().to_string();

    let ssh = env.dir.path().join("ssh");
    std::fs::write(
        &ssh,
        format!(
            "#!/bin/sh\nexec ssh -i '{}' -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR \"$@\"\n",
            key.display()
        ),
    )
    .unwrap();
    std::fs::set_permissions(&ssh, std::fs::Permissions::from_mode(0o700)).unwrap();
    let host = format!("ssh://root@127.0.0.1:{port}");
    let leon = |args: &[&str]| env.cmd().env("LEON_SSH", &ssh).arg("-H").arg(&host).args(args).output().unwrap();
    let ok = |args: &[&str]| {
        let o = leon(args);
        assert!(o.status.success(), "leon {args:?}: {}", String::from_utf8_lossy(&o.stderr));
        String::from_utf8(o.stdout).unwrap().trim().to_string()
    };
    let end = Instant::now() + Duration::from_secs(30);
    while !leon(&["ls"]).status.success() {
        assert!(Instant::now() < end, "sshd never answered: {}", String::from_utf8_lossy(&leon(&["ls"]).stderr));
        std::thread::sleep(Duration::from_millis(250));
    }
    ok(&["new", "r1"]);
    let b = ok(&["-s", "r1", "run", "--", "sh", "-c", "echo hello-remote; PS1='$ ' exec sh"]);

    let ssh_var = ssh.display().to_string();
    let h = Host::spawn(&env, &attach_then_stty(&format!("r1 -H {host}")), &[("LEON_SSH", &ssh_var)]);
    h.wait_text("hello-remote");

    docker(&["pause", &id]).expect("docker pause");
    let unpause = Cleanup(&["unpause"], id.clone());
    // the user stops to think, then types: the scenario's input, not a wait for output
    std::thread::sleep(Duration::from_millis(1500));
    h.send(b"echo during-pause\r");
    h.wait_for("the note that the keys weren't sent", |s| s.contains("not sent"));
    // as in the research run, the outage outlasts the watchdog: attach is relinking when it ends
    h.wait_for("attach to notice the link is down", |s| s.contains("reconnecting"));
    drop(unpause);
    let end = Instant::now() + Duration::from_secs(60);
    while h.screen().contains("reconnecting") {
        assert!(Instant::now() < end, "the link never came back:\n{}", h.screen());
        std::thread::sleep(Duration::from_millis(100));
    }

    h.send(b"echo back-$((6*7))\r");
    h.wait_text("back-42");
    let remote = ok(&["capture", "-b", &b]);
    assert!(!remote.contains("during-pause"), "the stalled keys never ran:\n{remote}");
    let line = remote.lines().find(|l| l.contains("back-$((6*7))")).unwrap_or_else(|| panic!("{remote}"));
    assert!(line.starts_with("$ echo back-"), "no fragment of the stalled keys before the next command: {line:?}");
    h.send(b"\x02d");
    assert_restored(&h, 0);
}
