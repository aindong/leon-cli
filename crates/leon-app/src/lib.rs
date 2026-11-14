//! The leon app: a GPUI window over an embedded leon server (docs/architecture.md).

pub mod embedded;
pub mod input;
pub mod render;
pub mod theme;
pub mod view;

/// Start the embedded server and run the app until it quits. Fails before any window opens if
/// the socket belongs to a server that holds sessions.
pub fn run() -> Result<(), String> {
    view::run(embedded::start()?);
    Ok(())
}
