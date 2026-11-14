//! A temp HOME with a Leon.app bundled from the test binary, and a runner for scripts/install.sh
//! that only ever sees that HOME.

#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::Command;

pub struct Kit {
    pub tmp: tempfile::TempDir,
    pub home: PathBuf,
    pub app: PathBuf,
}

pub struct Run {
    pub ok: bool,
    pub out: String,
}

impl Kit {
    pub fn new() -> Kit {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path().join("home");
        std::fs::create_dir(&home).unwrap();
        let app = tmp.path().join("build/Leon.app");
        let o = Command::new("bash")
            .arg(repo().join("scripts/bundle.sh"))
            .args(["--bin", env!("CARGO_BIN_EXE_leon"), "--out"])
            .arg(&app)
            .output()
            .unwrap();
        assert!(o.status.success(), "bundle.sh: {}", String::from_utf8_lossy(&o.stderr));
        Kit { tmp, home, app }
    }

    /// `install.sh --app <bundle> ARGS` with HOME set to the temp home.
    pub fn install_cmd(&self, args: &[&str]) -> Command {
        let mut c = Command::new("bash");
        c.arg(repo().join("scripts/install.sh"))
            .arg("--app")
            .arg(&self.app)
            .args(args)
            .env("HOME", &self.home)
            .env("PATH", "/usr/bin:/bin:/usr/sbin:/sbin");
        c
    }

    pub fn install(&self, args: &[&str]) -> Run {
        let o = self.install_cmd(args).output().unwrap();
        let out = String::from_utf8_lossy(&o.stdout).into_owned() + &String::from_utf8_lossy(&o.stderr);
        Run { ok: o.status.success(), out }
    }

    pub fn installed_app(&self) -> PathBuf {
        self.home.join("Applications/Leon.app")
    }

    pub fn cli(&self) -> PathBuf {
        self.home.join(".local/bin/leon")
    }
}

pub fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

pub fn bundle_id(app: &Path) -> Option<String> {
    let o = Command::new("plutil")
        .args(["-extract", "CFBundleIdentifier", "raw", "-o", "-"])
        .arg(app.join("Contents/Info.plist"))
        .output()
        .ok()?;
    o.status.success().then(|| String::from_utf8_lossy(&o.stdout).trim().to_string())
}

pub fn version(bin: &Path) -> Option<String> {
    let o = Command::new(bin).arg("--version").env("LEON_AUTOSTART", "0").output().ok()?;
    o.status.success().then(|| String::from_utf8_lossy(&o.stdout).trim().to_string())
}
