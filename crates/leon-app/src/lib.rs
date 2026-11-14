//! The leon app: a GPUI window over an embedded leon server (docs/architecture.md).

pub mod embedded;
pub mod input;
pub mod keymap;
pub mod palette;
pub mod panes;
pub mod render;
pub mod status;
pub mod theme;
pub mod view;

/// Start the embedded server and run the app until it quits. Fails before any window opens if
/// the socket belongs to a server that holds sessions.
pub fn run() -> Result<(), String> {
    view::run(embedded::start().inspect_err(|e| alert(e))?);
    Ok(())
}

/// Opened from Finder or the Dock there is no terminal to print to, so say why in an alert.
fn alert(why: &str) {
    use std::io::IsTerminal;
    // LaunchServices makes launchd (pid 1) the parent; a shell, a script or a test never is
    if std::io::stderr().is_terminal() || std::os::unix::process::parent_id() != 1 {
        return;
    }
    // shortcut: osascript's alert, since no GPUI window exists yet; use a GPUI prompt once the app
    // can open a window before taking the socket. The text goes in as argv, never as script source.
    let _ = std::process::Command::new("/usr/bin/osascript")
        .args(["-e", "on run argv", "-e", "display alert (item 1 of argv) message (item 2 of argv) as critical"])
        .args(["-e", "end run", "leon can't open", why])
        .status();
}
