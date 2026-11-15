//! The manifest: its signature, its format, and the rules a release must pass before leon takes
//! anything from it (docs/releases.md, "For an updater").

use std::collections::HashSet;

use minisign_verify::{Error, PublicKey, Signature};
use serde::Deserialize;

use crate::RELEASES;

/// One release, read from a manifest whose signature verified.
pub struct Release {
    pub version: String,
    /// The release's page, with its notes.
    pub notes_url: String,
    pub(crate) semver: semver::Version,
    min_os: MinOs,
    files: Vec<File>,
}

#[derive(Deserialize)]
struct Manifest {
    version: String,
    tag: String,
    #[serde(default)]
    notes_url: String,
    #[serde(default)]
    min_os: MinOs,
    files: Vec<File>,
}

/// The oldest systems the release's files run on; absent where it has no file for one.
#[derive(Deserialize, Default)]
struct MinOs {
    macos: Option<String>,
    linux_glibc: Option<String>,
}

/// One release file, as the manifest describes it.
#[derive(Deserialize)]
pub(crate) struct File {
    pub name: String,
    pub kind: String,
    pub os: String,
    pub arch: String,
    pub url: String,
    pub size: u64,
    pub sha256: String,
    pub signature: String,
}

/// The public keys in a keys file: one base64 key per line, `#` for comments.
pub(crate) fn keys(text: &str) -> Result<Vec<PublicKey>, String> {
    let lines = text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#'));
    let keys: Vec<PublicKey> = lines
        .map(|l| {
            PublicKey::from_base64(l).map_err(|_| "this leon's release keys hold a line that is not a key".to_string())
        })
        .collect::<Result<_, _>>()?;
    if keys.is_empty() {
        // a build from before the first key was pinned: it can verify nothing, so it takes nothing
        return Err(format!(
            "this leon was built with no release key, so it cannot check an update; update by hand: {RELEASES}"
        ));
    }
    Ok(keys)
}

/// `bytes` carry a signature (`signature`, a `.minisig` file's text) by one of `keys`. Only
/// pre-hashed signatures count: that is what `minisign -S` has made since 0.10.
pub(crate) fn verify(keys: &[PublicKey], what: &str, bytes: &[u8], signature: &str) -> Result<(), String> {
    let signature =
        Signature::decode(signature).map_err(|_| format!("{what}'s signature file is not a minisign signature"))?;
    let mut ours = false;
    for key in keys {
        match key.verify(bytes, &signature, false) {
            Ok(()) => return Ok(()),
            Err(Error::UnexpectedKeyId) => {}
            Err(_) => ours = true,
        }
    }
    Err(not_verified(what, ours))
}

/// Why a signature did not verify: `ours` when one of the pinned keys made it, over other bytes.
pub(crate) fn not_verified(what: &str, ours: bool) -> String {
    match ours {
        true => format!("{what} does not match its signature"),
        false => format!("{what} is signed by a key this leon does not trust"),
    }
}

/// Read a manifest whose signature has verified, and hold it to the format's rules.
pub(crate) fn read(bytes: &[u8]) -> Result<Release, String> {
    let odd = |e: serde_json::Error| format!("the release manifest is not what docs/releases.md describes: {e}");
    let value: serde_json::Value = serde_json::from_slice(bytes).map_err(odd)?;
    // a later schema may mean anything, so nothing else is read from it
    if value["schema"] != serde_json::json!(1) {
        return Err(format!("this leon is too old to read this release; update by hand: {RELEASES}"));
    }
    // fields this leon doesn't know are ignored: within schema 1 they are only ever added
    let m: Manifest = serde_json::from_value(value).map_err(odd)?;
    let semver = semver::Version::parse(&m.version)
        .map_err(|e| format!("the release manifest's version, \"{}\", is not a version: {e}", m.version))?;
    if m.tag != format!("v{}", m.version) {
        return Err(format!("the release manifest's tag, \"{}\", is not v plus its version, \"{}\"", m.tag, m.version));
    }
    let mut seen = HashSet::new();
    for f in &m.files {
        if !seen.insert((&f.kind, &f.os, &f.arch)) {
            return Err(format!("the release manifest lists two files for {} {} {}", f.kind, f.os, f.arch));
        }
        // a name is used as a file name and in a mirror's address: never a path
        let plain = f.name.bytes().all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b));
        if !plain || f.name.is_empty() || f.name == "." || f.name == ".." {
            return Err(format!("the release manifest has a file name that is not a plain file name: \"{}\"", f.name));
        }
        if f.sha256.len() != 64 || !f.sha256.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)) {
            return Err(format!("the release manifest's sha256 for {} is not one", f.name));
        }
    }
    Ok(Release { version: m.version, notes_url: m.notes_url, semver, min_os: m.min_os, files: m.files })
}

impl Release {
    /// The one file for `kind` on `os` and `arch`. With none, an error: never another platform's.
    pub(crate) fn file(&self, kind: &str, os: &str, arch: &str) -> Result<&File, String> {
        self.files
            .iter()
            .find(|f| (f.kind.as_str(), f.os.as_str(), f.arch.as_str()) == (kind, os, arch))
            .ok_or_else(|| format!("release {} has no {kind} file for {os} {arch}", self.version))
    }

    /// This machine's system is new enough for the release's files.
    pub(crate) fn runs_here(&self) -> Result<(), String> {
        let (system, floor) = match (std::env::consts::OS, &self.min_os) {
            ("macos", MinOs { macos: Some(floor), .. }) => ("macOS", floor),
            ("linux", MinOs { linux_glibc: Some(floor), .. }) => ("glibc", floor),
            _ => return Ok(()),
        };
        let have = crate::install::system_version().ok_or_else(|| {
            format!("leon {} needs {system} {floor} or newer, and this machine's is unknown", self.version)
        })?;
        let odd = |v: &str| format!("\"{v}\" is not a {system} version (leon {} needs {floor} or newer)", self.version);
        if parts(&have).ok_or_else(|| odd(&have))? < parts(floor).ok_or_else(|| odd(floor))? {
            return Err(format!("leon {} needs {system} {floor} or newer; this machine has {have}", self.version));
        }
        Ok(())
    }
}

/// A dotted version as numbers, without trailing zeros: "11.0" is [11], so it equals "11".
fn parts(version: &str) -> Option<Vec<u64>> {
    let mut parts = version.split('.').map(|p| p.parse().ok()).collect::<Option<Vec<u64>>>()?;
    while parts.last() == Some(&0) {
        parts.pop();
    }
    Some(parts)
}

#[cfg(test)]
mod tests {
    use super::parts;

    #[test]
    fn system_versions_compare_as_numbers() {
        assert!(parts("9.0") < parts("26.1"), "as text 9.0 would sort after 26.1");
        assert!(parts("2.100") > parts("2.36"), "as text 2.100 would sort before 2.36");
        assert_eq!(parts("11.0"), parts("11"));
        assert!(parts("11.0") < parts("11.0.1"));
        assert_eq!(parts("11.x"), None);
        assert_eq!(parts(""), None);
    }
}
