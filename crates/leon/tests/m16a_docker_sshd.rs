//! M16a criterion 15 (.agent/plans/2026-11-15-m16a-remote-ssh.md): the real thing: system ssh to
//! an sshd in a Linux container running a Linux `leon`. Skips (and says why) without Docker, a
//! Linux build (`LEON_LINUX_BIN`, else target/<arch>-unknown-linux-*/release/leon), or an image.

mod support;

use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, Instant};

use support::Env;

fn docker(args: &[&str]) -> Option<String> {
    let out = Command::new("docker").args(args).output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn linux_bin(arch: &str) -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("LEON_LINUX_BIN") {
        return Some(PathBuf::from(p)).filter(|p| p.exists());
    }
    let arch = if arch == "arm64" { "aarch64" } else { arch };
    let target = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target");
    ["gnu", "musl"]
        .iter()
        .map(|libc| target.join(format!("{arch}-unknown-linux-{libc}/release/leon")))
        .find(|p| p.exists())
}

struct Container(String);

impl Drop for Container {
    fn drop(&mut self) {
        let _ = Command::new("docker").args(["rm", "-f", &self.0]).output();
    }
}

#[test]
fn leon_over_real_ssh_to_a_linux_container() {
    let Some(arch) = docker(&["info", "--format", "{{.Architecture}}"]) else {
        eprintln!("skipped: no Docker");
        return;
    };
    let Some(bin) = linux_bin(&arch) else {
        eprintln!("skipped: no Linux leon for {arch} (build one, or set LEON_LINUX_BIN)");
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
    let tag = format!("leon-m16a-sshd-{}", std::process::id());
    if docker(&["build", "-q", "-t", &tag, ctx.to_str().unwrap()]).is_none() {
        eprintln!("skipped: the sshd image didn't build (offline?)");
        return;
    }
    let id = docker(&["run", "-d", "--rm", "-p", "127.0.0.1::22", &tag]).expect("docker run");
    let _c = Container(id.clone());
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
    let end = Instant::now() + Duration::from_secs(30);
    while !leon(&["ls"]).status.success() {
        assert!(Instant::now() < end, "sshd never answered: {}", String::from_utf8_lossy(&leon(&["ls"]).stderr));
        std::thread::sleep(Duration::from_millis(250));
    }
    let out = leon(&["new", "dock-1"]);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let ls = String::from_utf8(leon(&["ls"]).stdout).unwrap();
    assert!(ls.contains("dock-1"), "{ls}");
    assert!(!env.socket().exists(), "no local server");
}
