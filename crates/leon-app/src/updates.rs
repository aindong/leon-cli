//! Leon.app's updates (docs/updates.md): a look for a newer release at most once a day, a notice
//! when there is one, and part A's install (`leon_update`) when the user asks for it, never before.
//! No GPUI here: the workspace draws the notice (view_update.rs) and calls these, and every look
//! and install runs on a thread of its own, `leon-update`, which wakes the workspace when the
//! notice changes.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

use leon_update::{Check, Config, Method, Release, Target};
use serde::{Deserialize, Serialize};

/// How long the app waits between two looks of its own.
pub const DAY: u64 = 24 * 60 * 60;
/// What updates a Leon.app that Homebrew installed (docs/updates.md).
pub const CASK: &str = "brew upgrade --cask aindong/leon/leon";
/// System Settings, Privacy & Security, App Management.
pub const APP_MANAGEMENT: &str = "x-apple.systempreferences:com.apple.preference.security?Privacy_AppBundles";
pub use leon_update::RELEASES;

/// Where this app is installed, asked each time (the app's: `Target::running`).
pub type Find = Arc<dyn Fn() -> Result<Target, String> + Send + Sync>;
/// Called from the update thread when the notice changed.
pub type Wake = Arc<dyn Fn() + Send + Sync>;

/// What getting a release takes for this install.
#[derive(Debug, Clone, PartialEq)]
pub enum How {
    /// The app replaces itself (part A's `install`).
    Install,
    /// A package manager's: this command.
    Brew(String),
    /// macOS runs this app from a translocated copy, read-only: move it to Applications first.
    MoveFirst,
}

/// Where the updater is, which is what the notice says.
#[derive(Debug, Clone, PartialEq)]
pub enum Phase {
    /// Nothing to say: no look yet, up to date, or a look of its own that failed.
    Idle,
    /// A look the user asked for (Check for Updates) is running.
    Checking,
    /// What a look the user asked for found: this leon is the newest.
    UpToDate(String),
    NoRelease,
    /// A newer release, verified, with its release notes' address.
    Available {
        version: String,
        notes: String,
        how: How,
    },
    /// Installing it: the step `install` is on.
    Working {
        version: String,
        step: String,
    },
    /// Installed; it runs from the next launch.
    Done {
        version: String,
    },
    /// A look the user asked for, or an install, failed: why, as `leon_update` says it.
    Failed(String),
}

/// What a notice offers.
#[derive(Debug, Clone, PartialEq)]
pub enum Act {
    Install,
    /// Open the release notes.
    Notes(String),
    /// Put this command on the clipboard.
    Copy(String),
    Restart,
    /// Open System Settings at App Management.
    AppManagement,
    /// Open the release page, to update by hand.
    ReleasePage,
    Dismiss,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Notice {
    pub text: String,
    pub actions: Vec<Act>,
}

impl Phase {
    /// What the window shows for this phase; None draws nothing.
    pub fn notice(&self) -> Option<Notice> {
        let notice = |text: String, actions: Vec<Act>| Some(Notice { text, actions });
        match self {
            Phase::Idle => None,
            Phase::Checking => notice("Looking for a new release of leon…".into(), vec![]),
            Phase::UpToDate(v) => notice(format!("leon {v} is up to date."), vec![Act::Dismiss]),
            Phase::NoRelease => notice("There is no release of leon yet.".into(), vec![Act::Dismiss]),
            Phase::Available { version, notes, how } => {
                // the manifest's, which verified; still only a web page is opened
                let notes = notes.starts_with("https://").then(|| Act::Notes(notes.clone()));
                let (text, first) = match how {
                    How::Install => (format!("leon {version} is available."), Some(Act::Install)),
                    How::Brew(cmd) => (
                        format!("leon {version} is available. Homebrew updates this app: {cmd}"),
                        Some(Act::Copy(cmd.clone())),
                    ),
                    How::MoveFirst => (
                        format!(
                            "leon {version} is available. Move Leon.app to Applications first, then open it from \
                             there to install it."
                        ),
                        None,
                    ),
                };
                notice(text, first.into_iter().chain(notes).chain([Act::Dismiss]).collect())
            }
            Phase::Working { version, step } => notice(format!("Updating to leon {version}: {step}…"), vec![]),
            Phase::Done { version } => notice(
                format!("leon {version} is installed. It runs from the next launch."),
                vec![Act::Restart, Act::Dismiss],
            ),
            // the rename into place, refused by the system: App Management's, for an app replacing
            // an app in /Applications with no Team ID to match (docs/updates.md)
            Phase::Failed(why) if why.contains("can't replace") && why.contains("Operation not permitted") => notice(
                "macOS's App Management setting kept leon from replacing Leon.app. Allow it under System \
                 Settings, Privacy & Security, App Management, then install again, or update by hand."
                    .into(),
                vec![Act::AppManagement, Act::ReleasePage, Act::Dismiss],
            ),
            Phase::Failed(why) => notice(format!("The update failed: {why}"), vec![Act::ReleasePage, Act::Dismiss]),
        }
    }
}

/// macOS runs a quarantined app that was not moved (a download opened where it is) from a
/// random read-only copy under `AppTranslocation`, which no update can replace.
pub fn translocated(root: &Path) -> bool {
    root.components().any(|c| c.as_os_str() == "AppTranslocation")
}

/// What `updates.json` keeps across launches.
#[derive(Serialize, Deserialize, Default)]
struct Saved {
    /// When the last look of the app's own began, in seconds since the epoch.
    checked_at: Option<u64>,
    /// The version whose notice was dismissed.
    dismissed: Option<String>,
}

struct Inner {
    phase: Phase,
    /// The release `Available` is about, for Install.
    release: Option<Arc<Release>>,
    saved: Saved,
    /// A look or an install is running.
    busy: bool,
    /// Check for Updates came while the daily look ran: that look's answer is an asked one.
    asked: bool,
    /// The version this run installed: it waits for a restart, and no look replaces its notice.
    installed: Option<String>,
    generation: u64,
    last_error: Option<String>,
}

pub struct Updater {
    inner: Arc<Mutex<Inner>>,
    /// None keeps what would be saved in memory (tests).
    store: Option<PathBuf>,
    find: Find,
    wake: Wake,
}

/// A look of the app's own is due: none yet, a day since the last, or a clock that went back.
fn due(last: Option<u64>, now: u64) -> bool {
    last.is_none_or(|t| now >= t.saturating_add(DAY) || t > now.saturating_add(DAY))
}

/// A debug build is every test's: it looks on its own only when a test points it at a release
/// server, so no test run reaches the real one.
fn may_look_unasked() -> bool {
    !cfg!(debug_assertions) || ["LEON_RELEASE_URL", "LEON_TEST_UPDATE"].iter().any(|v| std::env::var_os(v).is_some())
}

impl Updater {
    /// An updater whose `updates.json` is `store`, for the install `find` gives.
    pub fn new(store: Option<PathBuf>, find: Find, wake: Wake) -> Updater {
        let saved = store.as_deref().and_then(|p| std::fs::read(p).ok());
        let saved = saved.and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
        let inner = Inner {
            phase: Phase::Idle,
            release: None,
            saved,
            busy: false,
            asked: false,
            installed: None,
            generation: 0,
            last_error: None,
        };
        Updater { inner: Arc::new(Mutex::new(inner)), store, find, wake }
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        lock(&self.inner)
    }

    /// The look of the app's own: when `enabled` (the `check_for_updates` setting) and due at
    /// `now`. The time is kept before the look starts, so one that fails, or an app that quits
    /// mid-look, waits a day like one that worked.
    pub fn daily(&self, enabled: bool, now: u64) {
        if !enabled || !may_look_unasked() {
            return;
        }
        let mut i = self.lock();
        if i.busy || i.installed.is_some() || !due(i.saved.checked_at, now) {
            return;
        }
        i.saved.checked_at = Some(now);
        save(self.store.as_deref(), &i.saved);
        drop(i);
        self.spawn(Job::Look { asked: false });
    }

    /// Check for Updates: a look now, whose answer the notice shows whatever it is.
    pub fn check_now(&self) {
        let mut i = self.lock();
        if i.busy {
            // an install shows its own steps; the daily look running now answers this check
            if !matches!(i.phase, Phase::Working { .. }) {
                (i.asked, i.phase) = (true, Phase::Checking);
                i.generation += 1;
                drop(i);
                (self.wake)();
            }
            return;
        }
        // installed already: what there is to say is that it waits for a restart
        if let Some(version) = i.installed.clone() {
            drop(i);
            return self.set_phase(Phase::Done { version });
        }
        drop(i);
        self.set_phase(Phase::Checking);
        self.spawn(Job::Look { asked: true });
    }

    /// Install Update: install the release the notice is about. One that a package manager owns
    /// or that runs translocated is never installed; with nothing found yet, look first.
    pub fn install(&self) {
        let i = self.lock();
        if i.busy {
            return;
        }
        match (&i.phase, i.release.clone()) {
            (Phase::Available { how: How::Install, .. }, Some(release)) => {
                drop(i);
                self.spawn(Job::Install(release));
            }
            (Phase::Available { .. } | Phase::Working { .. } | Phase::Done { .. }, _) => {}
            _ => {
                drop(i);
                self.check_now();
            }
        }
    }

    /// Hide the notice; one about a release stays hidden for that version, across launches.
    pub fn dismiss(&self) {
        let mut i = self.lock();
        if let Phase::Available { version, .. } = &i.phase {
            i.saved.dismissed = Some(version.clone());
            save(self.store.as_deref(), &i.saved);
        }
        if !matches!(i.phase, Phase::Working { .. } | Phase::Checking) {
            i.phase = Phase::Idle;
            i.generation += 1;
        }
    }

    /// What the window shows now.
    pub fn notice(&self) -> Option<Notice> {
        let i = self.lock();
        match &i.phase {
            Phase::Available { version, .. } if i.saved.dismissed.as_ref() == Some(version) => None,
            phase => phase.notice(),
        }
    }

    /// Moves with every change to what `notice` says.
    pub fn generation(&self) -> u64 {
        self.lock().generation
    }

    /// Why the last look or install failed, as the log line says it.
    pub fn last_error(&self) -> Option<String> {
        self.lock().last_error.clone()
    }

    /// Show `phase` (the tests' and the screenshots' way to a notice).
    pub fn set_phase(&self, phase: Phase) {
        set(&self.inner, &self.wake, phase);
    }

    fn spawn(&self, job: Job) {
        self.lock().busy = true;
        let (inner, find, wake, store) = (self.inner.clone(), self.find.clone(), self.wake.clone(), self.store.clone());
        let run = move || {
            // a bug in a job ends it like a failure, never with the updater stuck on busy
            let job = std::panic::AssertUnwindSafe(|| run(&inner, &find, &wake, store.as_deref(), job));
            let last = std::panic::catch_unwind(job)
                .unwrap_or_else(|_| Some(Phase::Failed("the update stopped on an error in leon".into())));
            // the job's answer and the end of `busy` in one step: an Install clicked the moment
            // the notice shows is taken, not dropped as "busy"
            {
                let mut i = lock(&inner);
                if let Some(Phase::Done { version }) = &last {
                    i.installed = Some(version.clone());
                }
                if let Some(last) = last {
                    i.phase = last;
                }
                (i.busy, i.asked) = (false, false);
                i.generation += 1;
            }
            wake();
        };
        if let Err(e) = std::thread::Builder::new().name("leon-update".into()).spawn(run) {
            eprintln!("leon: can't start the update thread: {e}");
            let mut i = self.lock();
            i.busy = false;
            if matches!(i.phase, Phase::Checking | Phase::Working { .. }) {
                i.phase = Phase::Failed(format!("can't start the update thread: {e}"));
                i.generation += 1;
            }
        }
    }
}

enum Job {
    /// `asked`: the user's Check for Updates, whose every answer is shown.
    Look {
        asked: bool,
    },
    Install(Arc<Release>),
}

/// One job, on the update thread. Returns the phase it ends in, which `spawn` shows; None leaves
/// the notice as it is (a look of the app's own speaks only when it finds a release).
fn run(inner: &Mutex<Inner>, find: &Find, wake: &Wake, store: Option<&Path>, job: Job) -> Option<Phase> {
    let cfg = Config::load();
    let target = find();
    // one line in the log; shown in the window only when the user asked for what failed
    let failed = |what: &str, why: String, shown: bool| {
        eprintln!("leon: the update {what} failed: {why}");
        lock(inner).last_error = Some(why.clone());
        shown.then_some(Phase::Failed(why))
    };
    match job {
        Job::Look { asked } => {
            let found = target.and_then(|t| leon_update::check(&cfg, &t).map(|c| (c, t)));
            // Check for Updates may have joined this look while it ran
            let asked = asked || lock(inner).asked;
            let (check, t) = match found {
                Ok(found) => found,
                Err(why) => return failed("check", why, asked),
            };
            match check {
                Check::Available(release) => {
                    let how = how(&t);
                    let mut i = lock(inner);
                    // asked for, a dismissed release is shown again: the user wants the answer
                    if asked && i.saved.dismissed.as_ref() == Some(&release.version) {
                        i.saved.dismissed = None;
                        save(store, &i.saved);
                    }
                    let phase =
                        Phase::Available { version: release.version.clone(), notes: release.notes_url.clone(), how };
                    i.release = Some(Arc::new(release));
                    Some(phase)
                }
                Check::UpToDate { .. } if asked => Some(Phase::UpToDate(cfg.version().to_string())),
                Check::NoRelease if asked => Some(Phase::NoRelease),
                _ => None,
            }
        }
        Job::Install(release) => {
            let version = release.version.clone();
            set(inner, wake, Phase::Working { version: version.clone(), step: "starting".into() });
            let installed = target.and_then(|t| {
                let mut say = |step: String| set(inner, wake, Phase::Working { version: version.clone(), step });
                leon_update::install(&cfg, &release, &t, &mut say)
            });
            match installed {
                Ok(()) => Some(Phase::Done { version }),
                Err(why) => failed("install", why, true),
            }
        }
    }
}

/// What getting a release takes for `t`. Asked only once there is a release: it may run `dpkg`.
fn how(t: &Target) -> How {
    if translocated(&t.root) {
        return How::MoveFirst;
    }
    match t.method() {
        Method::Homebrew { cask: true } => How::Brew(CASK.into()),
        Method::Homebrew { cask: false } => How::Brew("brew upgrade aindong/leon/leon".into()),
        // a .deb is Linux's, where there is no app; install says whose it is
        Method::Dpkg | Method::Own => How::Install,
    }
}

/// Show `phase`, then wake the workspace (after the lock is let go: it reads the notice).
fn set(inner: &Mutex<Inner>, wake: &Wake, phase: Phase) {
    {
        let mut i = lock(inner);
        i.phase = phase;
        i.generation += 1;
    }
    wake();
}

/// Write `updates.json` (0600, through a temp file and a rename) where the server keeps its
/// state; nothing when there is no store or no state dir yet.
fn save(store: Option<&Path>, saved: &Saved) {
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

fn lock(m: &Mutex<Inner>) -> MutexGuard<'_, Inner> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}
