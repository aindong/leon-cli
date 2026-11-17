//! Settings › Server › Tailscale: serving this Mac's leon over the tailnet. The tailnet-serve
//! work owns this file and fills it in; until then it draws nothing under its heading.

use super::*;

impl SettingsWindow {
    /// The Tailscale part of the Server page, under its heading.
    pub(super) fn tailscale_section(&self, _t: &Theme, _cx: &mut Context<SettingsWindow>) -> gpui::Div {
        div().debug_selector(|| "tailscale-section".into())
    }
}
