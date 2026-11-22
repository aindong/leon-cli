//! The Settings window's Shortcuts: the keymap's bindings, read-only, and the button that opens
//! init.lua.

use super::sections::matches;
use super::*;
use crate::keymap::{keys_text, Keymap};

/// One binding, as Shortcuts lists it.
#[derive(Debug, Clone, PartialEq)]
pub struct ShortcutRow {
    /// As docs/shortcuts.md writes them (`⌘⇧D`, `⌃B C`).
    pub keys: String,
    pub title: String,
    /// None: a Lua function.
    pub action: Option<String>,
}

impl ShortcutRow {
    pub(super) fn id(&self) -> String {
        format!("shortcut:{}", self.action.as_deref().unwrap_or(&self.keys))
    }

    pub(super) fn matches(&self, q: &str) -> bool {
        matches(q, &[&self.keys, &self.title, self.action.as_deref().unwrap_or("lua function"), "shortcuts"])
    }
}

/// Every binding of `km`, titled as the palette titles its action.
fn shortcut_rows(km: &Keymap) -> Vec<ShortcutRow> {
    km.bindings()
        .iter()
        .map(|b| {
            let title = match (&b.action, &b.args) {
                (None, _) => "Lua function".to_string(),
                (Some(a), args) => {
                    let named = crate::palette::BUILTINS.iter().find(|(n, _)| n == a).map(|(_, t)| t.to_string());
                    let named = named.or_else(|| km.actions.iter().find(|c| c.name == *a).map(|c| c.title.clone()));
                    match (a.as_str(), args) {
                        ("client.tab.goto", Some(args)) => format!("Go to Tab {}", args["index"]),
                        (_, Some(args)) => format!("{} {args}", named.unwrap_or_else(|| a.clone())),
                        _ => named.unwrap_or_else(|| a.clone()),
                    }
                }
            };
            ShortcutRow { keys: keys_text(&b.seq), title, action: b.action.clone() }
        })
        .collect()
}

/// `path` as a `file://` URL, with what a URL can't hold percent-encoded.
fn file_url(path: &Path) -> String {
    let mut url = String::from("file://");
    for b in path.to_string_lossy().bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'/' | b'-' | b'_' | b'.' | b'~' => url.push(b as char),
            b => url += &format!("%{b:02X}"),
        }
    }
    url
}

impl SettingsWindow {
    /// The keymap's bindings, as Shortcuts lists them.
    pub fn shortcuts(&self) -> Vec<ShortcutRow> {
        self.shortcuts.clone()
    }

    /// Ask the server for its keymap (init.lua's bindings over the defaults).
    pub(super) fn read_shortcuts(&mut self, cx: &mut Context<Self>) {
        let server = self.server.clone();
        cx.spawn(async move |this, cx| {
            let (tx, rx) = futures::channel::oneshot::channel();
            server.spawn("client.keymap", json!({}), move |r| drop(tx.send(r)));
            if let Ok(Ok(km)) = rx.await {
                let rows = shortcut_rows(&Keymap::app(&km));
                this.update(cx, |sw, cx| {
                    sw.shortcuts = rows;
                    cx.notify();
                })
                .ok();
            }
        })
        .detach();
    }

    /// Shortcuts' first line, with the button that opens init.lua.
    pub(super) fn shortcuts_intro(&self, t: &Theme, cx: &mut Context<Self>) -> gpui::Div {
        div()
            .flex()
            .items_center()
            .justify_between()
            .gap(px(16.))
            .child(Self::label(
                t,
                "Every key the app binds: leon's own, and init.lua's on top",
                "Change them with leon.bind in init.lua (docs/lua/keys.md)",
                false,
            ))
            .child(
                div()
                    .id("edit-init-lua")
                    .debug_selector(|| "edit-init-lua".into())
                    .flex_none()
                    .px(px(10.))
                    .py(px(4.))
                    .rounded(px(6.))
                    .border_1()
                    .border_color(rgb(t.border))
                    .cursor_pointer()
                    .hover(|d| d.bg(rgb(t.hover)))
                    .on_click(cx.listener(|sw, _, _, cx| {
                        // an editor can't open a file that isn't there: start an empty one, as
                        // `leon config set` would, folder and all
                        let made = sw.config.parent().map_or(Ok(()), std::fs::create_dir_all).and_then(|_| {
                            std::fs::OpenOptions::new().append(true).create(true).open(&sw.config).map(drop)
                        });
                        if let Err(e) = made {
                            sw.error = Some(format!("can't make {}: {e}", sw.config.display()));
                            cx.notify();
                        }
                        cx.open_url(&file_url(&sw.config));
                    }))
                    .child("Edit in init.lua"),
            )
    }

    /// The shortcuts at `rows` (indexes into `shortcuts`), one line each: keys, title and action.
    // shortcut: plain rows, not a uniform_list; the keymap is a few dozen bindings. Upgrade if a
    // config binds hundreds.
    pub(super) fn shortcut_rows(&self, rows: &[usize], t: &Theme) -> gpui::Div {
        let mut out = div().flex().flex_col();
        for r in rows.iter().filter_map(|i| self.shortcuts.get(*i)) {
            out = out.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(16.))
                    .py(px(4.))
                    .border_b_1()
                    .border_color(rgb(t.border))
                    .child(div().w(px(110.)).flex_none().text_color(rgb(t.title)).child(r.keys.clone()))
                    .child(div().flex_1().child(r.title.clone()))
                    .child(
                        div()
                            .text_size(px(SECONDARY_TEXT))
                            .text_color(rgb(t.muted))
                            .font_family("Menlo")
                            .child(r.action.clone().unwrap_or_else(|| "Lua function".into())),
                    ),
            );
        }
        out
    }
}
