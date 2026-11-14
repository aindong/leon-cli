//! The leon app: a GPUI window attached to a leon server over its socket (docs/architecture.md).

pub mod chrome;
pub mod embedded;
#[cfg(all(feature = "gui", target_os = "macos"))]
pub mod icons;
#[cfg(all(feature = "gui", target_os = "macos"))]
pub mod input;
pub mod keymap;
pub mod palette;
pub mod panes;
pub mod remote;
pub mod render;
pub mod scrollback;
pub mod status;
pub mod theme;
#[cfg(all(feature = "gui", target_os = "macos"))]
pub mod view;

/// Attach to the server on `socket` and run the app until it quits; the server keeps running.
#[cfg(all(feature = "gui", target_os = "macos"))]
pub fn run(socket: &std::path::Path) -> Result<(), String> {
    view::run(remote::Remote::connect(socket).inspect_err(|e| alert(e))?);
    Ok(())
}

/// Opened from Finder or the Dock there is no terminal to print to, so say why in an alert.
pub fn alert(why: &str) {
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
