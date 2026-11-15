//! The hosts list: `$LEON_STATE_DIR/hosts.json`, `{"version":1,"hosts":[{label, endpoints, icon}]}`
//! (docs/cli.md, `hosts add LABEL EP… --icon`). It names the machines `-H` may type into, so it
//! is read only from a private dir of ours and written 0600 through a temp file and a rename.

use std::io::Write;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::Endpoint;

pub const FILE: &str = "hosts.json";
/// How the error for a label in use starts.
pub const TAKEN: &str = "there is a host";
const VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Host {
    pub label: String,
    /// `ssh://…` endpoints. shortcut: `-H LABEL` uses the first; try them in order once a host
    /// with several (a LAN address and a VPN one) shows up.
    pub endpoints: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
}

impl Host {
    /// The first endpoint, parsed again: the file may have been edited by hand.
    pub fn endpoint(&self) -> Result<Endpoint, String> {
        let first = self.endpoints.first().ok_or_else(|| format!("host {} has no endpoint", self.label))?;
        first.parse()
    }
}

#[derive(Serialize, Deserialize)]
struct File {
    version: u32,
    hosts: Vec<Host>,
}

/// A label: letters, digits, `.`, `_`, `-`, at most 64, not starting with `-`.
pub fn check_label(label: &str) -> Result<(), String> {
    match crate::word(label, 64) {
        true => Ok(()),
        false => Err(format!("bad host label {label:?}: use letters, digits, . _ - (not starting with -)")),
    }
}

/// An icon name (the app looks it up): the same characters as a label.
pub fn check_icon(icon: &str) -> Result<(), String> {
    match crate::word(icon, 64) {
        true => Ok(()),
        false => Err(format!("bad icon name {icon:?}: use letters, digits, . _ -")),
    }
}

/// Add `host` to `list` as `leon hosts add` does: a good label and icon, every endpoint parseable,
/// no other host with that label. `list` is unchanged when it fails.
pub fn add(list: &mut Vec<Host>, host: Host) -> Result<(), String> {
    check_label(&host.label)?;
    if let Some(i) = &host.icon {
        check_icon(i)?;
    }
    if host.endpoints.is_empty() {
        return Err(format!("host {} has no endpoint", host.label));
    }
    for e in &host.endpoints {
        e.parse::<Endpoint>()?;
    }
    check_new_label(list, &host.label)?;
    list.push(host);
    Ok(())
}

/// A label that is good and not taken in `list`.
pub fn check_new_label(list: &[Host], label: &str) -> Result<(), String> {
    check_label(label)?;
    match list.iter().any(|h| h.label == label) {
        true => Err(format!("{TAKEN} {label:?} already; remove it first")),
        false => Ok(()),
    }
}

pub fn path() -> Result<PathBuf, String> {
    leon_core::paths::state_dir().map(|d| d.join(FILE)).ok_or_else(|| "no state dir: set LEON_STATE_DIR or HOME".into())
}

/// The hosts, oldest first; none when there is no file yet.
pub fn load() -> Result<Vec<Host>, String> {
    let path = path()?;
    let dir = path.parent().expect("a file in the state dir");
    if !dir.exists() {
        return Ok(Vec::new());
    }
    leon_core::paths::ensure_private_dir(dir).map_err(|e| e.to_string())?;
    let meta = match std::fs::symlink_metadata(&path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(format!("{}: {e}", path.display())),
        Ok(m) => m,
    };
    // SAFETY: geteuid has no preconditions.
    if !meta.file_type().is_file() || meta.uid() != unsafe { libc::geteuid() } {
        return Err(format!("{} isn't a regular file of yours", path.display()));
    }
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let file: File = serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    if file.version > VERSION {
        return Err(format!("{} is from a newer leon (version {})", path.display(), file.version));
    }
    Ok(file.hosts)
}

pub fn save(hosts: &[Host]) -> Result<(), String> {
    let path = path()?;
    let dir = path.parent().expect("a file in the state dir");
    leon_core::paths::ensure_private_dir(dir).map_err(|e| e.to_string())?;
    let tmp = dir.join(format!(".{FILE}.{}", std::process::id()));
    let _ = std::fs::remove_file(&tmp); // a leftover from a killed run with our pid
    let text = serde_json::to_string_pretty(&File { version: VERSION, hosts: hosts.to_vec() }).expect("json");
    let write = || -> std::io::Result<()> {
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(&tmp)?;
        f.write_all(text.as_bytes())?;
        f.write_all(b"\n")?;
        f.sync_all()?;
        std::fs::rename(&tmp, &path)
    };
    write().map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("can't save {}: {e}", path.display())
    })
}
