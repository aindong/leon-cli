//! leon's object model: sessions and windows with stable IDs, target resolution, and OSC 7501 program status.

pub mod error;
pub mod model;
pub mod paths;
pub mod program_status;
pub mod target;

pub use error::ApiError;
pub use model::{Session, State, Window};

/// A new, never-reused ID such as `session:7c9e…`.
pub fn new_id(kind: &str) -> String {
    format!("{kind}:{}", uuid::Uuid::new_v4())
}
