//! Unpacking a release archive into the staging directory. The archive's signature has verified
//! by now, and it is still unpacked as if it had not: entry by entry, refusing the whole archive
//! on the first entry with an absolute path, with `..`, of a kind that is not a file, directory or
//! link, with a link that leaves the directory, or that would be written through a link.

use std::path::{Component, Path, PathBuf};

/// The most an archive may unpack to. A release is tens of megabytes.
const MAX_UNPACKED: u64 = 1 << 30;

/// Unpack `archive` (named `name` in the release) into the empty directory `into`.
pub(crate) fn unpack(archive: &Path, name: &str, into: &Path) -> Result<(), String> {
    let io = |e: std::io::Error| format!("{name}: can't unpack it: {e}");
    let file = std::fs::File::open(archive).map_err(io)?;
    if name.ends_with(".tar.gz") {
        return tar_gz(file, name, into);
    }
    #[cfg(target_os = "macos")]
    if name.ends_with(".zip") {
        return zip(file, name, into);
    }
    Err(format!("{name}: leon does not know how to unpack this kind of file"))
}

fn refusing(name: &str, entry: &Path, why: &str) -> String {
    format!("{name}: refusing the entry \"{}\": {why}", entry.display())
}

/// Where the entry `path` goes under `into`: only plain names, and no parent that is a link.
fn destination(name: &str, into: &Path, path: &Path) -> Result<PathBuf, String> {
    let mut dest = into.to_path_buf();
    for part in path.components() {
        match part {
            Component::Normal(part) => {
                // nothing is written through a link, wherever it points
                if dest.symlink_metadata().is_ok_and(|m| m.file_type().is_symlink()) {
                    return Err(refusing(name, path, "it would be written through a link"));
                }
                dest.push(part);
            }
            Component::CurDir => {}
            Component::ParentDir => return Err(refusing(name, path, "it has \"..\" in its path")),
            Component::RootDir | Component::Prefix(_) => return Err(refusing(name, path, "it has an absolute path")),
        }
    }
    if dest == into {
        return Err(refusing(name, path, "it names the directory itself"));
    }
    Ok(dest)
}

/// A symlink at `path` may only point down from where it is: no `..` and no absolute target.
/// Then it cannot leave the archive's directory, whatever other links it passes through. (Counting
/// `..` against the path's depth is not enough: a link to `.` makes a deeper path that is not
/// deeper on disk.)
fn link_stays_inside(name: &str, path: &Path, target: &Path) -> Result<(), String> {
    match target.components().all(|part| matches!(part, Component::Normal(_) | Component::CurDir)) {
        true => Ok(()),
        false => {
            let why = format!("its link to \"{}\" could leave the archive's directory", target.display());
            Err(refusing(name, path, &why))
        }
    }
}

/// Count `more` bytes against the cap.
fn count(name: &str, total: &mut u64, more: u64) -> Result<(), String> {
    *total = total.saturating_add(more);
    match *total > MAX_UNPACKED {
        true => Err(format!("{name}: it unpacks to more than {MAX_UNPACKED} bytes")),
        false => Ok(()),
    }
}

fn tar_gz(file: std::fs::File, name: &str, into: &Path) -> Result<(), String> {
    use tar::EntryType;
    let io = |e: std::io::Error| format!("{name}: can't unpack it: {e}");
    let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(std::io::BufReader::new(file)));
    // the mode's executable bit is kept, without write access for others; the archive's owner
    // (root, in a release) is not
    archive.set_mask(0o022);
    let mut total = 0;
    for entry in archive.entries().map_err(io)? {
        let mut entry = entry.map_err(io)?;
        let path = entry.path().map_err(io)?.into_owned();
        destination(name, into, &path)?;
        let link = entry.link_name().map_err(io)?.map(|l| l.into_owned());
        match (entry.header().entry_type(), link) {
            (EntryType::Regular | EntryType::Continuous | EntryType::Directory, _) => {}
            (EntryType::XGlobalHeader, _) => continue, // notes for the whole archive, not a file
            (EntryType::Symlink, Some(target)) => link_stays_inside(name, &path, &target)?,
            // a hard link names another entry, from the top of the archive
            (EntryType::Link, Some(target)) => {
                destination(name, into, &target)
                    .map_err(|_| refusing(name, &path, "its hard link leaves the archive's directory"))?;
            }
            _ => return Err(refusing(name, &path, "it is not a file, a directory or a link")),
        }
        // the header's size is what tar reads for this entry, so it bounds what is written
        count(name, &mut total, entry.size())?;
        // tar checks again, against the real directories on disk; false means it skipped the entry
        if !entry.unpack_in(into).map_err(io)? {
            return Err(refusing(name, &path, "it would land outside the archive's directory"));
        }
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn zip(file: std::fs::File, name: &str, into: &Path) -> Result<(), String> {
    use std::io::Read;
    use std::os::unix::ffi::OsStrExt;
    use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
    let io = |e: std::io::Error| format!("{name}: can't unpack it: {e}");
    let odd = |e: zip::result::ZipError| format!("{name}: can't unpack it: {e}");
    let mut archive = zip::ZipArchive::new(file).map_err(odd)?;
    let mut total = 0;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(odd)?;
        // the name as the archive spells it: the checks are ours, not the zip crate's
        let path = PathBuf::from(std::ffi::OsStr::from_bytes(entry.name_raw()));
        // `ditto` puts a file's extended attributes in a "._name" entry beside it (or under
        // __MACOSX). leon restores none, and written out as files they would break the app's seal.
        let sidecar = path.file_name().is_some_and(|n| n.as_bytes().starts_with(b"._"));
        if sidecar || path.starts_with("__MACOSX") {
            continue;
        }
        let dest = destination(name, into, &path)?;
        // 0755 whatever the umask: under a loose one the installed app's directories would be
        // writable by the user's whole group
        let mkdir = |dir: &Path| std::fs::DirBuilder::new().recursive(true).mode(0o755).create(dir).map_err(io);
        if entry.is_dir() {
            mkdir(&dest)?;
            continue;
        }
        if let Some(parent) = dest.parent() {
            mkdir(parent)?;
        }
        if entry.is_symlink() {
            let mut target = String::new();
            (&mut entry).take(4096).read_to_string(&mut target).map_err(io)?;
            link_stays_inside(name, &path, Path::new(&target))?;
            std::os::unix::fs::symlink(&target, &dest).map_err(io)?;
            continue;
        }
        // create_new: an archive that names one path twice does not overwrite, or follow a link
        let mut out = std::fs::OpenOptions::new().write(true).create_new(true).open(&dest).map_err(io)?;
        let wrote = std::io::copy(&mut (&mut entry).take(MAX_UNPACKED + 1), &mut out).map_err(io)?;
        count(name, &mut total, wrote)?;
        let mode = entry.unix_mode().unwrap_or(0o644) & 0o755;
        out.set_permissions(std::fs::Permissions::from_mode(mode)).map_err(io)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn links_only_point_down() {
        let ok = |path: &str, target: &str| link_stays_inside("a", Path::new(path), Path::new(target)).is_ok();
        assert!(ok("top/link", "README.md"));
        assert!(ok("top/link", "./docs/cli.md"));
        assert!(ok("Leon.app/Contents/Frameworks/X.framework/X", "Versions/Current/X"));
        // any step up is refused, even one that would stay inside: with a link to "." in the
        // archive, "a/../.." is two steps up on disk and only one by counting
        for up in ["..", "../b/c", "a/../..", "../..", "a/../../.."] {
            assert!(!ok("top/a/link", up), "{up}");
        }
        assert!(!ok("top/link", "/etc/passwd"));
    }

    #[test]
    fn destinations_are_plain_paths_under_the_directory() {
        let into = Path::new("/staging");
        let to = |path: &str| destination("a", into, Path::new(path));
        assert_eq!(to("top/leon").unwrap(), Path::new("/staging/top/leon"));
        assert_eq!(to("./top/./leon").unwrap(), Path::new("/staging/top/leon"));
        for bad in ["/etc/passwd", "../x", "top/../../x", "top/..", ".", ""] {
            assert!(to(bad).is_err(), "{bad:?}");
        }
    }
}
