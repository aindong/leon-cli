//! `updates.json`: what Leon.app's updates keep across launches (updates.rs).

use std::path::Path;

use serde::{Deserialize, Serialize};

/// What `updates.json` keeps across launches.
#[derive(Serialize, Deserialize, Default)]
pub(super) struct Saved {
    /// When the last look of the app's own began, in seconds since the epoch.
    pub checked_at: Option<u64>,
    /// The version whose notice was dismissed.
    pub dismissed: Option<String>,
    /// The version that last ran: a launch of another one says it was updated.
    pub seen: Option<String>,
    /// The version that showed the agent skill's notice (view_skill.rs).
    pub skill_notice: Option<String>,
}

/// Write `updates.json` (0600, through a temp file and a rename) where the server keeps its
/// state; nothing when there is no store or no state dir yet.
pub(super) fn save(store: Option<&Path>, saved: &Saved) {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let Some(path) = store else { return };
    let write = || -> std::io::Result<()> {
        let dir = path.parent().filter(|d| d.is_dir()).ok_or_else(|| std::io::Error::other("no state dir"))?;
        leon_core::paths::ensure_private_dir(dir)?;
        let tmp = path.with_extension("json.tmp");
        let _ = std::fs::remove_file(&tmp);
        let mut f = std::fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(&tmp)?;
        f.write_all(&serde_json::to_vec(saved).map_err(std::io::Error::other)?)?;
        std::fs::rename(&tmp, path)
    };
    if let Err(e) = write() {
        eprintln!("leon: can't save {}: {e}", path.display());
    }
}
