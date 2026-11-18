//! The Settings window's Tailscale section: the `serve_tailscale` switch, the port and tags it
//! serves with (a field, and a tag list with a field to add one), and what the server says about
//! serving (`server.status`'s `tailnet`), fetched every 2 s while the window is open
//! (docs/architecture/tailnet.md).

use std::time::Duration;

use leon_lua::settings::{Settings, MOST_TAGS, TAILNET_PORT};

use super::patterns::{typed, Typed};
use super::*;

/// What `leon.set(key, value)` says of `value`: the setting's own check.
fn check(key: &str, value: &Value) -> Result<(), String> {
    Settings::default().set(key, value, Path::new(""))
}

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
        let tags = match self.tags().is_empty() {
            true => "no tagged machines".to_string(),
            false => format!("tagged machines with {}", self.tags().join(", ")),
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
                    "Your own machines signed in to the same Tailscale account, and {tags}. They can do anything \
                     you can here. Reach it with leon -H tailnet://THIS-MACHINE."
                ),
                t.muted,
            ))
            .children(state)
            .children(refused)
            .child(self.port_control(t, cx))
            .child(self.tags_control(t, cx))
            .child(self.toggle(t, "serve_web", "Also serve them a view-only web page", cx))
    }

    /// Why the last port typed, and the last tag, were refused.
    pub fn tailscale_errors(&self) -> [Option<String>; 2] {
        [self.port.error.clone(), self.tag.error.clone()]
    }

    /// `serve_tailscale_tags`, as init.lua has them.
    fn tags(&self) -> Vec<String> {
        let list = self.row("serve_tailscale_tags").and_then(|r| r.value.as_array().cloned()).unwrap_or_default();
        list.iter().filter_map(|t| t.as_str().map(String::from)).collect()
    }

    /// The port's field, showing the port while empty.
    fn port_control(&self, t: &Theme, cx: &mut Context<Self>) -> gpui::Div {
        let row = self.row("serve_tailscale_port");
        let locked = Self::is_locked(&row);
        let port = row.as_ref().map_or(json!(TAILNET_PORT), |r| r.value.clone());
        let note = format!("The port it listens on, 1 to 65535; {TAILNET_PORT} unless set");
        div()
            .flex()
            .flex_col()
            .gap(px(6.))
            .child(Self::label(t, "Port", &note, locked))
            .when(!locked, |d| {
                d.child(self.port.draw(
                    t,
                    "port",
                    port.to_string(),
                    "Set",
                    Self::port_key,
                    |sw, cx| sw.set_port(cx),
                    cx,
                ))
            })
            .children(Self::locked(t, "serve_tailscale_port", &row))
    }

    /// The tags (× removes one) and a field to add one.
    fn tags_control(&self, t: &Theme, cx: &mut Context<Self>) -> gpui::Div {
        let row = self.row("serve_tailscale_tags");
        let locked = Self::is_locked(&row);
        let list = self.tags();
        let note = format!("Tagged machines it lets in besides your own; at most {MOST_TAGS}");
        let mut out = div().flex().flex_col().gap(px(6.)).child(Self::label(t, "Tags", &note, locked));
        for (i, tag) in list.iter().enumerate() {
            let rest: Vec<String> = list.iter().enumerate().filter(|&(j, _)| j != i).map(|(_, t)| t.clone()).collect();
            out = out.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        div()
                            .flex_1()
                            .px(px(8.))
                            .py(px(4.))
                            .rounded(px(6.))
                            .bg(rgb(t.bg))
                            .font_family("Menlo")
                            .child(tag.clone()),
                    )
                    .child(rows::button(t, format!("tag-remove-{i}"), "×", !locked, cx, move |sw, cx| {
                        sw.choose("serve_tailscale_tags", json!(rest), cx)
                    })),
            );
        }
        if !locked {
            let hint = "Add a tag, such as tag:ci".to_string();
            out = out.child(self.tag.draw(t, "tag", hint, "Add", Self::tag_key, |sw, cx| sw.add_tag(cx), cx));
        }
        out.children(Self::locked(t, "serve_tailscale_tags", &row))
    }

    fn port_key(&mut self, e: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        match typed(&mut self.port.draft, e, cx) {
            Typed::Enter => self.set_port(cx),
            Typed::Escape => self.port.escape(window, &self.focus),
            Typed::Edited | Typed::Other => {}
        }
    }

    fn tag_key(&mut self, e: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        match typed(&mut self.tag.draft, e, cx) {
            Typed::Enter => self.add_tag(cx),
            Typed::Escape => self.tag.escape(window, &self.focus),
            Typed::Edited | Typed::Other => {}
        }
    }

    /// Write the typed port once the setting takes it; otherwise say why under the field.
    fn set_port(&mut self, cx: &mut Context<Self>) {
        let draft = self.port.draft.trim().to_string();
        // a whole number goes as one, anything else as the text typed, for the setting to refuse
        let port = draft.parse::<u64>().map_or_else(|_| json!(draft), |n| json!(n));
        self.write_field("serve_tailscale_port", port, |sw| &mut sw.port, cx);
    }

    /// Add the typed tag once the list with it is a setting's; otherwise say why under the field.
    fn add_tag(&mut self, cx: &mut Context<Self>) {
        let (mut list, draft) = (self.tags(), self.tag.draft.trim().to_string());
        if list.contains(&draft) {
            // already let in: nothing to write
            (self.tag.draft, self.tag.error) = (String::new(), None);
            return cx.notify();
        }
        list.push(draft);
        self.write_field("serve_tailscale_tags", json!(list), |sw| &mut sw.tag, cx);
    }

    /// Write `value` from `field` when `key` is writable and takes it, emptying the field; or show
    /// the setting's refusal under it and write nothing.
    fn write_field(
        &mut self,
        key: &str,
        value: Value,
        field: fn(&mut Self) -> &mut patterns::TextField,
        cx: &mut Context<Self>,
    ) {
        if Self::is_locked(&self.row(key)) {
            return;
        }
        let f = field(self);
        if let Err(why) = check(key, &value) {
            f.error = Some(why);
            return cx.notify();
        }
        (f.draft, f.error) = (String::new(), None);
        self.choose(key, value, cx);
    }
}
