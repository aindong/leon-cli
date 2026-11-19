//! A key's echo shown at once, not at GPUI's next display tick (ADR R2 as amended,
//! docs/architecture/gpui-patch.md): when the screen of the pane a key went to arrives, AppKit is
//! asked to display GPUI's view, and GPUI's `displayLayer:` draws that frame as this run-loop pass
//! commits and presents it with the transaction. Once per key, so output with no key behind it
//! (a flood) keeps to the display link.

use gpui::Window;

use super::*;

/// The pane a key went to whose echo is not shown yet, and how many were shown at once (tests).
#[derive(Default)]
pub(super) struct Echo {
    pane: Option<String>,
    shows: u64,
}

impl Workspace {
    /// A key went to pane `id`: its next screen is shown at once.
    pub(super) fn key_sent(&mut self, id: &str) {
        self.echo.pane = Some(id.to_string());
    }

    /// Pushed screens `ids` (This Mac's, already given to their grids): the one a key is waiting
    /// on is shown at once (the real window only: a test window has no NSView).
    pub(super) fn echo_arrived(&mut self, ids: &[String], window: &Window) {
        if self.echo.pane.as_ref().is_some_and(|p| ids.contains(p)) {
            self.echo.pane = None;
            self.echo.shows += 1;
            if self.platform {
                show_now(window);
            }
        }
    }

    /// How many screens were shown at once for a key's echo.
    pub fn echo_shows(&self) -> u64 {
        self.echo.shows
    }
}

/// Ask AppKit to display the window's GPUI view at the end of this run-loop pass; false (nothing
/// asked) when the window is not on screen, where GPUI draws nothing, or has no NSView.
pub fn show_now(window: &Window) -> bool {
    use objc2::msg_send;
    use objc2::runtime::{AnyObject, Bool};
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    let Ok(RawWindowHandle::AppKit(h)) = HasWindowHandle::window_handle(window).map(|h| h.as_raw()) else {
        return false;
    };
    let view: *mut AnyObject = h.ns_view.as_ptr().cast();
    // SAFETY: GPUI's NSView lives as long as its window, and this runs on the main thread
    unsafe {
        let ns_window: *mut AnyObject = msg_send![view, window];
        let state: usize = msg_send![ns_window, occlusionState];
        if state & (1 << 1) == 0 {
            return false; // not NSWindowOcclusionStateVisible
        }
        let _: () = msg_send![view, setNeedsDisplay: Bool::YES];
    }
    true
}
