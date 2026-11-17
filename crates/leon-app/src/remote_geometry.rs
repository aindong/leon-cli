//! How this app draws a window, told to its server (`client.geometry`), so a float or drawer made
//! there starts its program at the size the app will give it instead of 24x80.

use super::*;
use leon_core::overlays::Geometry;

impl Shared {
    /// Tell the server again what was last told, on a re-attach: it may have restarted, or missed
    /// it while a link was down. Off the UI's connection, as a re-attach's other calls are.
    /// Returns what it told.
    pub(super) fn resend_geometry(&self) -> Option<Value> {
        // not held over the call: over a link it can take seconds, and the UI reports through it
        let last = lock(&self.geometry).clone();
        if let Some(p) = &last {
            let _ = lock(&self.fetch).call("client.geometry", p.clone());
        }
        last
    }
}

impl Remote {
    /// Tell the server window `window_id`'s geometry, unless it was the last told. A server too
    /// old for the method refuses it, and its popups keep starting at 24x80.
    pub fn report_geometry(&self, session_id: &str, window_id: &str, g: Geometry) {
        let p = json!({ "session_id": session_id, "window_id": window_id, "area": g.area, "cell": g.cell,
                        "float_frame": g.float_frame, "drawer_frame": g.drawer_frame });
        let mut last = lock(&self.shared.geometry);
        if last.as_ref() == Some(&p) {
            return;
        }
        *last = Some(p.clone());
        drop(last);
        // a failure needs nothing: a re-attach tells the last one again (so it isn't forgotten
        // here), and a refusal (a server too old for it) won't change
        self.send_then("client.geometry", p, |_| {});
    }
}
