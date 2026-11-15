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
pub mod target;
pub mod terminal;
pub mod theme;

pub use error::ApiError;
pub use model::{Block, Place, Session, State, Window};

/// A new, never-reused ID such as `session:7c9e…`.
pub fn new_id(kind: &str) -> String {
    format!("{kind}:{}", uuid::Uuid::new_v4())
}
