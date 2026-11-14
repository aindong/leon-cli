//! Telling the user outside the window: notifications and the Dock badge (macOS), and clicks on
//! a notification coming back as a place to go. The decisions are `attention::Notifier`'s.

use std::sync::{Arc, Mutex, MutexGuard};

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
static WAKE: Mutex<Option<Wake>> = Mutex::new(None);

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// A notification was clicked (any thread): queue where it goes and wake the window.
pub fn clicked(c: Click) {
    lock(&CLICKS).push(c);
    let wake = lock(&WAKE).clone();
    if let Some(w) = wake {
        w();
    }
}

/// The clicks since the last call, oldest first.
pub(crate) fn take_clicks() -> Vec<Click> {
    std::mem::take(&mut *lock(&CLICKS))
}

/// What `clicked` wakes: the window's poll.
pub(crate) fn on_click(wake: Wake) {
    *lock(&WAKE) = Some(wake);
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
