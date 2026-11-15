//! leon's updater: find the newest release, check it, and replace this install with it.
//!
//! A release is described by a signed manifest, `latest.json` (docs/releases.md). Everything here
//! follows that page's rules for an updater: the manifest is verified against the keys built into
//! this binary before anything is read from it, a release is taken only if it is newer, and a
//! file only if its sha256 is the manifest's and its own signature verifies. docs/updates.md is
//! the user's side of it.
//!
//! `leon self-update` and the app call the same three things: [`Target::running`] (how this leon
//! was installed), [`check`] (is there a newer release) and [`install`].

mod fetch;
mod install;
mod manifest;
mod stage;
mod unpack;

pub use install::{install, tidy, Method, Target};
pub use manifest::Release;

/// Where a person gets a release by hand.
pub const RELEASES: &str = "https://github.com/aindong/leon-cli/releases/latest";
/// The manifest's address, which never changes (docs/releases.md).
const ADDRESS: &str = "https://github.com/aindong/leon-cli/releases/latest/download/latest.json";
/// The keys releases are signed with, as they were when this binary was built.
const PINNED_KEYS: &str = include_str!("../../leon/release-keys.pub");
/// The most a manifest and its signature may be: far above a real one, far below a full disk.
const MANIFEST_LIMIT: u64 = 1 << 20;
const SIGNATURE_LIMIT: u64 = 4 << 10;

/// What this leon trusts and where it looks for releases.
pub struct Config {
    keys: String,
    version: String,
    address: String,
    /// `LEON_RELEASE_URL`: a directory that holds a whole release, files and manifest.
    mirror: Option<String>,
    /// Seconds to wait for a connection, or for a stalled answer to move again.
    timeout: u64,
    #[cfg(debug_assertions)]
    pause: Option<String>,
}

impl Config {
    /// The one place the environment is read. `LEON_RELEASE_URL` moves where releases come from,
    /// which is safe only because every check below still runs against the built-in keys.
    pub fn load() -> Config {
        let mirror = std::env::var("LEON_RELEASE_URL").ok().map(|m| m.trim_end_matches('/').to_string());
        let mirror = mirror.filter(|m| !m.is_empty());
        let address = mirror.as_ref().map_or(ADDRESS.to_string(), |m| format!("{m}/latest.json"));
        #[allow(unused_mut)] // only the test seam changes it
        let mut cfg = Config {
            keys: PINNED_KEYS.to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            address,
            mirror,
            timeout: 30,
            #[cfg(debug_assertions)]
            pause: None,
        };
        #[cfg(debug_assertions)]
        cfg.test_seam();
        cfg
    }

    /// The tests' way in: a throwaway key, another version, a short timeout, a place to stop.
    /// It is compiled only into debug builds, so a release build has nothing that can change the
    /// keys it trusts (the update_release_build test builds one and looks).
    #[cfg(debug_assertions)]
    fn test_seam(&mut self) {
        let Ok(text) = std::env::var("LEON_TEST_UPDATE") else { return };
        let seam: serde_json::Value = serde_json::from_str(&text).expect("LEON_TEST_UPDATE is JSON");
        if let Some(path) = seam["keys"].as_str() {
            self.keys = std::fs::read_to_string(path).expect("LEON_TEST_UPDATE's keys file");
        }
        if let Some(version) = seam["version"].as_str() {
            self.version = version.to_string();
        }
        if let (Some(address), None) = (seam["address"].as_str(), &self.mirror) {
            self.address = address.to_string();
        }
        if let Some(secs) = seam["timeout_secs"].as_u64() {
            self.timeout = secs;
        }
        self.pause = seam["pause"].as_str().map(str::to_string);
    }

    /// The version this leon is.
    pub fn version(&self) -> &str {
        &self.version
    }

    /// Where a release file is fetched from: the mirror when one is set, else the manifest's url.
    pub(crate) fn url_of(&self, file: &manifest::File) -> String {
        match &self.mirror {
            Some(mirror) => format!("{mirror}/{}", file.name),
            None => file.url.clone(),
        }
    }

    /// A point in an install where a test can stop it (debug builds only).
    pub(crate) fn step(&self, name: &str) {
        #[cfg(debug_assertions)]
        if self.pause.as_deref() == Some(name) {
            eprintln!("leon: paused at {name} (LEON_TEST_UPDATE)");
            loop {
                std::thread::sleep(std::time::Duration::from_secs(3600));
            }
        }
        let _ = name;
    }
}

/// What a look at the release server found.
pub enum Check {
    /// The manifest's address answers 404: nothing has been released.
    NoRelease,
    /// The newest release is this version, or older.
    UpToDate { latest: String },
    /// A newer release, verified, that this machine can run.
    Available(Release),
}

/// Fetch the manifest and its signature, verify them, and say whether the release is newer.
/// Errors are one line each, ready to show.
pub fn check(cfg: &Config) -> Result<Check, String> {
    let keys = manifest::keys(&cfg.keys)?;
    let (status, bytes) = fetch::get(cfg, &cfg.address, MANIFEST_LIMIT, "latest.json")?;
    match status {
        200 => {}
        404 => return Ok(Check::NoRelease),
        other => return Err(format!("the release server answered {other} for {}", cfg.address)),
    }
    let signature_at = format!("{}.minisig", cfg.address);
    let (status, signature) = fetch::get(cfg, &signature_at, SIGNATURE_LIMIT, "latest.json.minisig")?;
    match status {
        200 => {}
        404 => return Err(format!("latest.json has no signature: {signature_at} is not there")),
        other => return Err(format!("the release server answered {other} for {signature_at}")),
    }
    manifest::verify(&keys, "latest.json", &bytes, &String::from_utf8_lossy(&signature))?;
    // only now is anything read from it
    let release = manifest::read(&bytes)?;
    let running = semver::Version::parse(&cfg.version)
        .map_err(|e| format!("this leon's own version, {}, is not a version: {e}", cfg.version))?;
    if release.semver.cmp_precedence(&running) != std::cmp::Ordering::Greater {
        return Ok(Check::UpToDate { latest: release.version });
    }
    release.runs_here()?;
    Ok(Check::Available(release))
}
