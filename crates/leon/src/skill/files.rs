//! Writing into the user's agent config: atomic replaces and backups that never lose a file.

use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

/// How a write saves the file it replaces.
#[derive(Clone, Copy)]
pub enum Backup {
    /// Once, before leon's first change: settings files, where the user's content stays in the file,
    /// so the first backup is the pristine original.
    Once,
    /// Every time, under a new name: a foreign SKILL.md that is replaced whole.
    Fresh,
}

pub fn backup_path(p: &Path) -> PathBuf {
    with_suffix(p, ".leon-backup")
}

fn with_suffix(p: &Path, suffix: &str) -> PathBuf {
    let mut name = p.file_name().unwrap_or_default().to_os_string();
    name.push(suffix);
    p.with_file_name(name)
}

/// Copy `path` to `<path>.leon-backup` (with `Fresh`, the first free `.leon-backup.N`). The backup
/// is created new (never through a link) with mode 0600: settings files can hold API keys.
pub fn back_up(path: &Path, how: Backup) -> io::Result<()> {
    let mut src = File::open(path)?;
    for n in 1.. {
        let b = if n == 1 { backup_path(path) } else { with_suffix(path, &format!(".leon-backup.{n}")) };
        let opened =
            OpenOptions::new().write(true).create_new(true).mode(0o600).custom_flags(libc::O_NOFOLLOW).open(&b);
        match opened {
            Ok(mut f) => {
                return io::copy(&mut src, &mut f).map(drop).inspect_err(|_| {
                    let _ = std::fs::remove_file(&b); // a half backup would pass for the original later
                });
            }
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => match how {
                Backup::Once => return Ok(()),
                Backup::Fresh => continue,
            },
            Err(e) => return Err(e),
        }
    }
    unreachable!()
}

/// Where a write to `path` lands: with `follow`, a symlink's target (even a dangling one), so a
/// settings file linked from a dotfiles repo stays linked; without, the path itself.
fn destination(path: &Path, follow: bool) -> PathBuf {
    if !follow {
        return path.to_path_buf();
    }
    std::fs::canonicalize(path)
        .or_else(|_| std::fs::read_link(path).map(|t| path.parent().unwrap_or(Path::new("")).join(t)))
        .unwrap_or_else(|_| path.to_path_buf())
}

/// Replace the file by renaming a fresh temp file (random name, 0600) over it, keeping the old
/// file's permissions. A failed write leaves the old file whole and removes the temp file.
pub fn write_atomic(path: &Path, bytes: &[u8], follow: bool) -> io::Result<()> {
    let real = destination(path, follow);
    let dir = real.parent().expect("a file path");
    std::fs::create_dir_all(dir)?;
    let mut tmp = tempfile::NamedTempFile::new_in(dir)?;
    if let Ok(meta) = std::fs::metadata(&real) {
        tmp.as_file().set_permissions(meta.permissions())?;
    }
    tmp.write_all(bytes)?;
    tmp.as_file().sync_all()?;
    tmp.persist(&real).map_err(|e| e.error)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_backups_never_overwrite_and_once_keeps_the_first() {
        let d = tempfile::tempdir().unwrap();
        let f = d.path().join("SKILL.md");
        for text in ["a", "b"] {
            std::fs::write(&f, text).unwrap();
            back_up(&f, Backup::Fresh).unwrap();
        }
        assert_eq!(std::fs::read_to_string(backup_path(&f)).unwrap(), "a");
        assert_eq!(std::fs::read_to_string(d.path().join("SKILL.md.leon-backup.2")).unwrap(), "b");
        std::fs::write(&f, "c").unwrap();
        back_up(&f, Backup::Once).unwrap();
        assert_eq!(std::fs::read_to_string(backup_path(&f)).unwrap(), "a");
    }

    #[test]
    fn a_dangling_link_is_written_through() {
        let d = tempfile::tempdir().unwrap();
        let link = d.path().join("settings.json");
        std::os::unix::fs::symlink("real.json", &link).unwrap();
        write_atomic(&link, b"{}", true).unwrap();
        assert!(std::fs::symlink_metadata(&link).unwrap().file_type().is_symlink());
        assert_eq!(std::fs::read_to_string(d.path().join("real.json")).unwrap(), "{}");
        write_atomic(&link, b"[]", false).unwrap();
        assert!(!std::fs::symlink_metadata(&link).unwrap().file_type().is_symlink(), "no follow replaces the link");
    }
}
