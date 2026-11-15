//! The staging directory: `.leon-update` beside the install, so that what an update unpacks is on
//! the install's filesystem and can be renamed into place. One update at a time holds a lock on
//! it. Nothing is ever cleared by matching a name: only a directory this user made, of exactly
//! that name, and only while the lock is held.

use std::fs::{DirBuilder, File, OpenOptions};
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::os::unix::io::AsRawFd;
use std::path::{Path, PathBuf};

use crate::RELEASES;

pub(crate) const NAME: &str = ".leon-update";

pub(crate) struct Staging {
    pub dir: PathBuf,
    /// Open on `dir`, and holding its lock until this drops.
    lock: File,
}

impl Staging {
    /// Make (or take over) the staging directory in `parent`, lock it, and empty it.
    pub(crate) fn take(parent: &Path) -> Result<Staging, String> {
        let dir = parent.join(NAME);
        match DirBuilder::new().mode(0o700).create(&dir) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            // no install it cannot finish: say so now, before anything is downloaded
            Err(e) => return Err(cant_write(parent, &e.to_string())),
        }
        let in_the_way =
            || format!("{} is in the way: it is not a staging directory leon made; move it away", dir.display());
        // O_NOFOLLOW: a symlink of that name is someone else's, and so is whatever it points at
        let lock =
            OpenOptions::new().read(true).custom_flags(libc::O_NOFOLLOW | libc::O_DIRECTORY).open(&dir).map_err(
                |e| match e.kind() {
                    // an update that just finished removed it between our create and our open
                    std::io::ErrorKind::NotFound => format!("{} went away; run the update again", dir.display()),
                    _ => in_the_way(),
                },
            )?;
        let meta = lock.metadata().map_err(|_| in_the_way())?;
        // SAFETY: geteuid has no preconditions.
        if !meta.is_dir() || meta.uid() != unsafe { libc::geteuid() } {
            return Err(in_the_way());
        }
        // SAFETY: the fd is open for the life of `lock`.
        if unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
            let e = std::io::Error::last_os_error();
            return Err(match e.kind() {
                std::io::ErrorKind::WouldBlock => {
                    format!("another update is running in {}; wait for it to finish", dir.display())
                }
                _ => format!("can't lock {}: {e}", dir.display()),
            });
        }
        // the directory we locked is still the one at that path (an update that just finished
        // removes its own)
        let same = std::fs::symlink_metadata(&dir).is_ok_and(|now| (now.dev(), now.ino()) == (meta.dev(), meta.ino()));
        if !same {
            return Err(format!("{} changed while leon was taking it; run the update again", dir.display()));
        }
        let io = |e: std::io::Error| format!("can't clear {}: {e}", dir.display());
        lock.set_permissions(std::fs::Permissions::from_mode(0o700)).map_err(io)?;
        // what a killed update left: ours to remove, now that the lock says nobody is using it
        clear(&dir).map_err(io)?;
        Ok(Staging { dir, lock })
    }
}

/// The refusal for a place leon may not write: it never reaches for more rights.
pub(crate) fn cant_write(path: &Path, why: &str) -> String {
    format!(
        "can't write to {}: {why}. leon never asks for a password: run this as a user who can write there, or update by hand from {RELEASES}",
        path.display()
    )
}

/// Remove everything in `dir`, following no symlink. A directory without write access for its
/// owner (an old app's, an archive's) is opened up first, or it would block every later update.
fn clear(dir: &Path) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700))?;
            clear(&path)?;
            std::fs::remove_dir(&path)?;
        } else {
            std::fs::remove_file(&path)?;
        }
    }
    Ok(())
}

impl Drop for Staging {
    fn drop(&mut self) {
        // while the lock is still held; `lock` closes after this
        let _ = clear(&self.dir).and_then(|()| std::fs::remove_dir(&self.dir));
        let _ = &self.lock;
    }
}
