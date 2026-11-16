//! How this leon was installed, and replacing it: download into the staging directory, check the
//! file, unpack it, run the new leon once, and rename it into place. Until that rename the old
//! install is untouched, and the rename is one system call, so an update killed at any point
//! leaves a leon that runs.

use std::io::Read;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use sha2::{Digest, Sha256};

use crate::manifest::{self, File};
use crate::stage::Staging;
use crate::{Config, Release};

/// What marks an app bundle as leon's (scripts/bundle.sh writes it).
const BUNDLE_ID: &[u8] = b"<string>dev.leon.app</string>";

/// Who updates this leon.
#[derive(Debug, PartialEq)]
pub enum Method {
    /// Homebrew installed it: `brew upgrade`, with `--cask` for the app.
    Homebrew { cask: bool },
    /// It came from a `.deb`: dpkg owns the file.
    Dpkg,
    /// A tarball, `scripts/install.sh`, or an app someone unzipped: leon replaces itself.
    Own,
}

/// The running leon: what it is and what an update would replace.
pub struct Target {
    /// `app` for a leon inside `X.app/Contents/MacOS`, else `cli`.
    pub kind: &'static str,
    /// What an update replaces: the app bundle, or the binary. A real path, with no symlinks.
    pub root: PathBuf,
    exe: PathBuf,
}

impl Target {
    /// The running binary, by its real path: what is on PATH may be a symlink to it.
    pub fn running() -> Result<Target, String> {
        let exe = std::env::current_exe().and_then(|p| p.canonicalize());
        let exe = exe.map_err(|e| format!("can't find the running leon on disk: {e}"))?;
        // X.app/Contents/MacOS/leon
        let bundle = exe.ancestors().nth(3).filter(|app| {
            app.extension().is_some_and(|e| e == "app")
                && exe.strip_prefix(app).is_ok_and(|p| p.starts_with("Contents/MacOS"))
        });
        let (kind, root) = match bundle {
            Some(app) => ("app", app.to_path_buf()),
            None => ("cli", exe.clone()),
        };
        // only leon's own app is leon's to swap out, as scripts/install.sh and bundle.sh hold:
        // inside another app, a new Leon.app in its place would delete that app
        let plist = std::fs::read(root.join("Contents/Info.plist")).unwrap_or_default();
        if kind == "app" && !plist.windows(BUNDLE_ID.len()).any(|w| w == BUNDLE_ID) {
            return Err(format!(
                "this leon is inside {}, which is not a Leon.app (bundle id dev.leon.app); leon will not replace another app",
                root.display()
            ));
        }
        Ok(Target { kind, root, exe })
    }

    /// Who updates this leon. Homebrew's and dpkg's installs are found here, and leon never
    /// replaces them. Asked only once there is something to install: it may run `dpkg`.
    pub fn method(&self) -> Method {
        let path = self.exe.to_string_lossy();
        if path.contains("/Cellar/leon/") {
            return Method::Homebrew { cask: false };
        }
        if path.contains("/Caskroom/leon/") || (self.kind == "app" && cask_points_at(&self.root)) {
            return Method::Homebrew { cask: true };
        }
        if dpkg_owns(&self.exe) {
            return Method::Dpkg;
        }
        Method::Own
    }
}

/// Homebrew moves a cask's app to /Applications and leaves `Caskroom/leon/<version>/Leon.app`, a
/// symlink to it. `HOMEBREW_PREFIX` is what `brew shellenv` sets; the rest are its defaults.
fn cask_points_at(app: &Path) -> bool {
    let prefixes = std::env::var_os("HOMEBREW_PREFIX")
        .map(PathBuf::from)
        .into_iter()
        .chain(["/opt/homebrew", "/usr/local", "/home/linuxbrew/.linuxbrew"].map(PathBuf::from));
    let entries = |dir: PathBuf| std::fs::read_dir(dir).into_iter().flatten().flatten().map(|e| e.path());
    prefixes
        .flat_map(|prefix| entries(prefix.join("Caskroom/leon")))
        .flat_map(entries)
        .any(|link| link.canonicalize().is_ok_and(|real| real == app))
}

/// `dpkg -S PATH` succeeds when a package owns the file. dpkg from PATH; where PATH has none,
/// from where Debian keeps it.
fn dpkg_owns(exe: &Path) -> bool {
    let ask = |dpkg: &str| Command::new(dpkg).arg("-S").arg(exe).stdin(std::process::Stdio::null()).output();
    ask("dpkg").or_else(|_| ask("/usr/bin/dpkg")).is_ok_and(|o| o.status.success())
}

/// This platform as the manifest names it.
pub(crate) fn platform() -> (&'static str, &'static str) {
    let arch = match std::env::consts::ARCH {
        "aarch64" => "arm64",
        other => other,
    };
    (std::env::consts::OS, arch)
}

/// This machine's macOS version, or its glibc version on Linux; None where neither can be asked.
pub(crate) fn system_version() -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        let mut buf = [0u8; 64];
        let mut len = buf.len();
        // SAFETY: the name is NUL-terminated, and buf and len describe one writable buffer.
        let rc = unsafe {
            libc::sysctlbyname(
                c"kern.osproductversion".as_ptr(),
                buf.as_mut_ptr().cast(),
                &mut len,
                std::ptr::null_mut(),
                0,
            )
        };
        let text = std::str::from_utf8(buf.get(..len)?).ok()?.trim_end_matches('\0');
        (rc == 0 && !text.is_empty()).then(|| text.to_string())
    }
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    {
        // SAFETY: glibc returns a pointer to a static NUL-terminated string.
        let version = unsafe { std::ffi::CStr::from_ptr(libc::gnu_get_libc_version()) };
        version.to_str().ok().map(str::to_string)
    }
    #[cfg(not(any(target_os = "macos", all(target_os = "linux", target_env = "gnu"))))]
    None
}

impl Release {
    /// The `.deb`'s address for this machine, for a leon dpkg owns.
    pub fn deb_url(&self, cfg: &Config) -> Result<String, String> {
        self.file("deb", "linux", platform().1).map(|f| cfg.url_of(f))
    }
}

/// Clear what a killed update left beside an install leon owns. Nothing to do is not an error.
pub fn tidy(target: &Target) {
    let parent = target.root.parent().unwrap_or(Path::new("/"));
    let left = parent.join(crate::stage::NAME).symlink_metadata().is_ok_and(|m| m.is_dir());
    if left && target.method() == Method::Own {
        let _ = Staging::take(parent); // dropping it removes it
    }
}

/// Replace `target` with `release`. `say` gets a line for each slow step. Only for
/// `Method::Own`: the caller sends Homebrew's and dpkg's installs to their own tools.
pub fn install(cfg: &Config, release: &Release, target: &Target, say: &mut dyn FnMut(String)) -> Result<(), String> {
    // whoever calls: a package manager's install is never replaced
    if target.method() != Method::Own {
        return Err(format!("{} belongs to a package manager, which updates it", target.root.display()));
    }
    let (os, arch) = platform();
    let file = release.file(target.kind, os, arch)?;
    let keys = manifest::keys(&cfg.keys)?;
    let parent = target.root.parent().ok_or("the running leon has no directory")?;
    // an app trades places with the new one, which needs write access to the app itself: one
    // that root or another user installed is refused here, before anything is downloaded
    if target.kind == "app" && !writable(&target.root) {
        return Err(crate::stage::cant_write(&target.root, "Permission denied"));
    }
    let staging = Staging::take(parent)?;

    let (archive, url) = (staging.dir.join("download"), cfg.url_of(file));
    say(format!("downloading {} ({:.1} MB)", file.name, file.size as f64 / 1e6));
    match crate::fetch::download(cfg, &url, file.size, &file.name, &archive)? {
        200 => {}
        other => return Err(format!("the release server answered {other} for {}: {url}", file.name)),
    }
    // nothing of the archive is read, let alone unpacked, before both of these hold
    verify(&keys, file, &archive)?;

    let unpacked = staging.dir.join("unpacked");
    std::fs::create_dir(&unpacked).map_err(|e| format!("can't write to {}: {e}", staging.dir.display()))?;
    crate::unpack::unpack(&archive, &file.name, &unpacked)?;
    cfg.step("unpacked");

    // what a release holds (docs/releases.md): Leon.app at the zip's top, or leon in the
    // tarball's one directory
    let (new, leon) = match target.kind {
        "app" => (unpacked.join("Leon.app"), unpacked.join("Leon.app/Contents/MacOS/leon")),
        _ => {
            let leon = unpacked.join(file.name.trim_end_matches(".tar.gz")).join("leon");
            (leon.clone(), leon)
        }
    };
    // a real file at that very path: no link stands in for the binary, the bundle, or a
    // directory between them
    let is_program = |m: std::fs::Metadata| m.is_file() && m.permissions().mode() & 0o100 != 0;
    let direct = leon.canonicalize().ok()
        == unpacked.canonicalize().ok().and_then(|u| Some(u.join(leon.strip_prefix(&unpacked).ok()?)));
    if !direct || !leon.symlink_metadata().is_ok_and(is_program) {
        return Err(format!("{} holds no leon where a release keeps it", file.name));
    }
    // the new leon runs on this machine and is the version the manifest promised
    let ran =
        Command::new(&leon).arg("--version").env("LEON_AUTOSTART", "0").stdin(std::process::Stdio::null()).output();
    // its first line: the version, or why it could not start
    let first = |bytes: &[u8]| String::from_utf8_lossy(bytes).lines().next().unwrap_or_default().to_string();
    let (ran_well, says) = match ran {
        Ok(o) if o.stdout.is_empty() => (false, first(&o.stderr)),
        Ok(o) => (o.status.success(), first(&o.stdout)),
        Err(e) => (false, e.to_string()),
    };
    if !ran_well || says.split_whitespace().nth(1) != Some(&release.version) {
        return Err(format!(
            "the leon in {} does not run as leon {} here (it said \"{says}\")",
            file.name, release.version
        ));
    }

    // on disk before it replaces the old one: a crash just after the rename must not leave a
    // leon that is half written
    std::fs::File::open(&leon)
        .and_then(|f| f.sync_all())
        .map_err(|e| format!("can't write {}: {e}", leon.display()))?;
    cfg.step("installing");
    swap(&new, &target.root)
        .map_err(|e| format!("can't replace {}: {e}; nothing was changed", target.root.display()))?;
    // the rename itself, on disk; nothing to undo if this fails
    let _ = std::fs::File::open(parent).and_then(|dir| dir.sync_all());
    cfg.step("installed");
    Ok(()) // dropping the staging directory removes it, with the old app in it
}

/// The downloaded file has the manifest's size and sha256, and its own signature verifies. The
/// signature alone would not do: an older release's file is validly signed too.
fn verify(keys: &[minisign_verify::PublicKey], file: &File, archive: &Path) -> Result<(), String> {
    let io = |e: std::io::Error| format!("can't read the download of {}: {e}", file.name);
    let signature = minisign_verify::Signature::decode(&file.signature)
        .map_err(|_| format!("{}'s signature in the manifest is not a minisign signature", file.name))?;
    // one pass for the sha256 and for each key that could have made the signature
    let mut verifiers: Vec<_> = keys.iter().filter_map(|k| k.verify_stream(&signature).ok()).collect();
    let (mut sha, mut size, mut chunk) = (Sha256::new(), 0u64, vec![0u8; 1 << 20]);
    let mut input = std::fs::File::open(archive).map_err(io)?;
    loop {
        let n = match input.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(io(e)),
        };
        size += n as u64;
        sha.update(&chunk[..n]);
        verifiers.iter_mut().for_each(|v| v.update(&chunk[..n]));
    }
    let sum: String = sha.finalize().iter().map(|b| format!("{b:02x}")).collect();
    if size != file.size || sum != file.sha256 {
        return Err(format!("{}: its sha256 is not the one in the manifest ({size} bytes, {sum})", file.name));
    }
    let ours = !verifiers.is_empty();
    match verifiers.iter_mut().any(|v| v.finalize().is_ok()) {
        true => Ok(()),
        false => Err(manifest::not_verified(&file.name, ours)),
    }
}

/// This user may write to `path` (access(2), so ACLs and read-only mounts count).
fn writable(path: &Path) -> bool {
    use std::os::unix::ffi::OsStrExt;
    let Ok(path) = std::ffi::CString::new(path.as_os_str().as_bytes()) else { return false };
    // SAFETY: a NUL-terminated path that lives past the call.
    unsafe { libc::access(path.as_ptr(), libc::W_OK) == 0 }
}

/// Put `new` where `old` is, in one system call.
fn swap(new: &Path, old: &Path) -> std::io::Result<()> {
    if !new.is_dir() {
        return std::fs::rename(new, old); // a file over a file: atomic on every Unix
    }
    // two directories trade places, so there is never a moment without an app; the old one ends
    // up in the staging directory and goes with it
    #[cfg(target_os = "macos")]
    {
        use std::os::unix::ffi::OsStrExt;
        let c = |p: &Path| std::ffi::CString::new(p.as_os_str().as_bytes()).map_err(std::io::Error::other);
        let (from, to) = (c(new)?, c(old)?);
        // SAFETY: both are NUL-terminated paths that live past the call.
        match unsafe { libc::renamex_np(from.as_ptr(), to.as_ptr(), libc::RENAME_SWAP) } {
            0 => Ok(()),
            _ => Err(std::io::Error::last_os_error()),
        }
    }
    #[cfg(not(target_os = "macos"))]
    Err(std::io::Error::other("only macOS has an app to replace"))
}
