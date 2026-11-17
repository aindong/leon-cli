//! The Settings window's Tailscale section: the `serve_tailscale` switch, the port and tags it
//! serves with, and what the server says about serving (`server.status`'s `tailnet`), fetched
//! every 2 s while the window is open (docs/architecture/tailnet.md).

use std::time::Duration;

use super::*;

/// How often the window asks the server how serving goes.
const POLL: Duration = Duration::from_secs(2);

impl SettingsWindow {
    /// Ask the server for `server.status` now and every `POLL` until the window closes; render
    /// only draws what came back (R5, R6, R8 of docs/architecture/gpui.md).
    pub(super) fn watch_tailnet(&mut self, cx: &mut Context<Self>) {
        let server = self.server.clone();
        cx.spawn(async move |this, cx| loop {
            let (tx, rx) = futures::channel::oneshot::channel();
            server.spawn("server.status", json!({}), move |r| drop(tx.send(r)));
            let status = rx.await.ok().and_then(Result::ok).map(|s| s["tailnet"].clone());
            let open = this.update(cx, |sw, cx| {
                if sw.tailnet != status {
                    sw.tailnet = status;
                    cx.notify();
                }
            });
            if open.is_err() {
                return; // the window closed
            }
            cx.background_executor().timer(POLL).await;
        })
        .detach();
    }

    /// `server.status`'s `tailnet`, as last fetched.
    pub fn tailnet(&self) -> Option<Value> {
        self.tailnet.clone()
    }

    pub(super) fn tailscale_section(&self, t: &Theme, cx: &mut Context<SettingsWindow>) -> gpui::Div {
        let text = |v: &Value| v.as_str().unwrap_or("").to_string();
        let note = |s: String, color: u32| div().text_size(px(SECONDARY_TEXT)).text_color(rgb(color)).child(s);
        let port = self.row("serve_tailscale_port").map_or(json!(leon_lua::settings::TAILNET_PORT), |r| r.value);
        let tags: Vec<String> = self
            .row("serve_tailscale_tags")
            .and_then(|r| r.value.as_array().cloned())
            .unwrap_or_default()
            .iter()
            .map(text)
            .collect();
        let tags = match tags.is_empty() {
            true => "no tagged machines".to_string(),
            false => format!("tagged machines with {}", tags.join(", ")),
        };
        let t_ = self.tailnet.clone().unwrap_or(Value::Null);
        let state = match text(&t_["state"]).as_str() {
            "serving" => {
                let at: Vec<String> = t_["addresses"].as_array().into_iter().flatten().map(text).collect();
                Some(note(format!("Serving on {} for {}", at.join(", "), text(&t_["login"])), t.text))
            }
            "stopped" => Some(note(format!("Not serving: {}", text(&t_["reason"])), t.error)),
            _ => None,
        };
        let refused = t_["last_refusal"]
            .as_object()
            .map(|r| note(format!("Last refused {}: {}", text(&r["peer"]), text(&r["reason"])), t.muted));
        Self::subtitle(t, "Tailscale")
            .child(self.toggle(t, "serve_tailscale", "Let my other machines on Tailscale reach this server", cx))
            .child(note(
                format!(
                    "Port {port}; your own machines signed in to the same Tailscale account, and {tags}. They can do \
                     anything you can here. Reach it with leon -H tailnet://THIS-MACHINE."
                ),
                t.muted,
            ))
            .children(state)
            .children(refused)
    }
}
