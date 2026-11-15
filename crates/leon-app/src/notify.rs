//! Telling the user outside the window: notifications and the Dock badge (macOS), and clicks on
//! a notification coming back as a place to go. The decisions are `attention::Notifier`'s.

use std::sync::{Arc, Mutex, MutexGuard, Weak};

use crate::attention::Notice;

/// Where a clicked notification came from.
#[derive(Debug, Clone, PartialEq)]
pub struct Click {
    pub host_label: String,
    pub session_id: String,
    /// Empty for the notice a storm folded into one: that click opens Activity.
    pub block_id: String,
}

type Wake = Arc<dyn Fn() + Send + Sync>;

static CLICKS: Mutex<Vec<Click>> = Mutex::new(Vec::new());
/// Clicks kept for a window to take; past this the oldest go (one no window knows waits that long).
const MAX_CLICKS: usize = 64;
/// Every window's wake, for as long as its window keeps it.
static WAKES: Mutex<Vec<Weak<dyn Fn() + Send + Sync>>> = Mutex::new(Vec::new());

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// A notification was clicked (any thread): queue where it goes and wake every window, since the
/// one that knows the pane need not be the one made last.
pub fn clicked(c: Click) {
    let mut q = lock(&CLICKS);
    q.push(c);
    let over = q.len().saturating_sub(MAX_CLICKS);
    q.drain(..over);
    drop(q);
    let wakes: Vec<Wake> = lock(&WAKES).iter().filter_map(Weak::upgrade).collect();
    wakes.iter().for_each(|w| w());
}

/// The clicks that `mine` picks, oldest first; the rest stay for the window they belong to (the
/// queue is the process's, and tests run windows side by side).
pub(crate) fn take_clicks(mine: impl Fn(&Click) -> bool) -> Vec<Click> {
    let mut q = lock(&CLICKS);
    let (taken, kept) = std::mem::take(&mut *q).into_iter().partition(mine);
    *q = kept;
    taken
}

/// What `clicked` wakes: a window's poll, until the window drops `wake`.
pub(crate) fn on_click(wake: &Wake) {
    let mut wakes = lock(&WAKES);
    wakes.retain(|w| w.strong_count() > 0); // windows that have closed
    wakes.push(Arc::downgrade(wake));
}

/// Post `n` as a system notification. Only a bundled app can (macOS refuses a bare binary), so
/// `./target/release/leon app` says so once and posts nothing. Do Not Disturb and Focus are the
/// system's to apply.
pub fn post(n: &Notice) {
    #[cfg(target_os = "macos")]
    mac::post(n);
    #[cfg(not(target_os = "macos"))]
    let _ = n; // shortcut: notifications are macOS only; Linux (M15) can post through D-Bus later
}

/// Set up notifications at launch: macOS asks the user once, and a notice posted before the
/// answer is lost.
pub fn prepare() {
    #[cfg(target_os = "macos")]
    mac::prepare();
}

/// The Dock icon's badge: `text`, or none.
pub fn set_badge(text: Option<&str>) {
    #[cfg(target_os = "macos")]
    mac::badge(text);
    #[cfg(not(target_os = "macos"))]
    let _ = text;
}

#[cfg(target_os = "macos")]
#[path = "notify_mac.rs"]
mod mac;
