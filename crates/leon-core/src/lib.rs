//! leon's object model: sessions and windows with stable IDs, target resolution, and OSC 7501 program status.

pub mod error;
pub mod events;
pub mod hints;
pub mod keys;
pub mod layout;
pub mod model;
pub mod mouse;
pub mod overlays;
pub mod panes;
pub mod paths;
pub mod program_status;
pub mod screen;
pub mod search;
pub mod spawn;
pub mod target;
pub mod terminal;
pub mod theme;

pub use error::ApiError;
pub use model::{Block, Place, Session, State, Window, MAX_BLOCKS};

/// The wait env var `name` sets, in whole seconds, else `default` (unset, 0, not a number, or not
/// shorter). For the tests' knobs (`LEON_LINK_HOLD_SECS` and the like): they only shorten a wait.
pub fn secs_from_env(name: &str, default: std::time::Duration) -> std::time::Duration {
    let secs = std::env::var(name).ok().and_then(|s| s.parse::<u64>().ok()).filter(|&s| s > 0);
    secs.map_or(default, |s| std::time::Duration::from_secs(s).min(default))
}

/// A new, never-reused ID such as `session:7c9e…`.
pub fn new_id(kind: &str) -> String {
    format!("{kind}:{}", uuid::Uuid::new_v4())
}
