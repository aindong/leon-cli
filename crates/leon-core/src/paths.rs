use std::path::{Path, PathBuf};

/// Where the server's socket lives: `$LEON_RUNTIME_DIR`, else `$XDG_RUNTIME_DIR/leon`, else
/// `<home>/.local/state/leon/run`, where home is `$HOME` or the passwd entry. Never a shared
/// directory such as /tmp unless the user names one (which `ensure_private_dir` then refuses).
pub fn runtime_dir() -> PathBuf {
    if let Some(d) = std::env::var_os("LEON_RUNTIME_DIR").filter(|d| !d.is_empty()) {
        return PathBuf::from(d);
    }
    if let Some(d) = std::env::var_os("XDG_RUNTIME_DIR").filter(|d| !d.is_empty()) {
        return PathBuf::from(d).join("leon");
    }
    match home() {
        Some(h) => h.join(".local/state/leon/run"),
        // No home at all: a per-user dir that ensure_private_dir still has to find safe.
        None => PathBuf::from(format!("/tmp/leon-{}", unsafe { libc::geteuid() })),
    }
}

pub fn home() -> Option<PathBuf> {
    if let Some(h) = std::env::var_os("HOME").filter(|h| !h.is_empty()) {
        return Some(PathBuf::from(h));
    }
    // SAFETY: getpwuid_r writes into our buffer; the returned pointer is only read before the buffer drops.
    unsafe {
        let mut pw: libc::passwd = std::mem::zeroed();
        let mut out: *mut libc::passwd = std::ptr::null_mut();
        let mut buf = vec![0 as libc::c_char; 4096];
        let rc = libc::getpwuid_r(libc::geteuid(), &mut pw, buf.as_mut_ptr(), buf.len(), &mut out);
        if rc != 0 || out.is_null() || pw.pw_dir.is_null() {
            return None;
        }
        let dir = std::ffi::CStr::from_ptr(pw.pw_dir).to_string_lossy().into_owned();
        (!dir.is_empty()).then(|| PathBuf::from(dir))
    }
}

pub fn socket_path() -> PathBuf {
    runtime_dir().join("server.sock")
}

/// Make sure `dir` is a private directory of ours: create it (0700) if it's missing, and refuse
/// one that is a symlink, owned by someone else, or open to group/others. Nothing is ever
/// chmod'ed. Like tmux, a socket in a directory others can write to could be hijacked.
pub fn ensure_private_dir(dir: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::{DirBuilderExt, MetadataExt};
    if !dir.exists() {
        std::fs::DirBuilder::new().recursive(true).mode(0o700).create(dir)?;
    }
    let meta = std::fs::symlink_metadata(dir)?;
    let unsafe_because = if !meta.file_type().is_dir() {
        Some("is not a directory (or is a symlink)")
    } else if meta.uid() != unsafe { libc::geteuid() } {
        Some("is owned by another user")
    } else if meta.mode() & 0o077 != 0 {
        Some("is readable or writable by other users (needs mode 0700)")
    } else {
        None
    };
    match unsafe_because {
        Some(why) => Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            format!("unsafe runtime directory {}: it {why}", dir.display()),
        )),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::PermissionsExt;

    use super::*;

    fn mode(p: &Path) -> u32 {
        std::fs::metadata(p).unwrap().permissions().mode() & 0o777
    }

    #[test]
    fn a_new_dir_is_created_private() {
        let parent = tempfile::tempdir().unwrap();
        let fresh = parent.path().join("leon/run");
        ensure_private_dir(&fresh).unwrap();
        assert_eq!(mode(&fresh), 0o700);
    }

    #[test]
    fn an_open_dir_is_refused_and_left_alone() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o755)).unwrap();
        let err = ensure_private_dir(dir.path()).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::PermissionDenied);
        assert_eq!(mode(dir.path()), 0o755, "never chmod a directory leon didn't create");
    }

    #[test]
    fn a_symlinked_dir_is_refused() {
        let parent = tempfile::tempdir().unwrap();
        let real = parent.path().join("real");
        std::fs::DirBuilder::new().create(&real).unwrap();
        std::fs::set_permissions(&real, std::fs::Permissions::from_mode(0o700)).unwrap();
        let link = parent.path().join("link");
        std::os::unix::fs::symlink(&real, &link).unwrap();
        assert!(ensure_private_dir(&link).is_err());
    }
}
