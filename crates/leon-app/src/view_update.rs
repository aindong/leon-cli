//! The update notice (updates.rs): a small card at the window's bottom right, over the panes. It
//! is not a modal and takes no focus: the keys stay with the workspace.

use super::*;
use crate::palette::Item;
use crate::updates::{Act, Find, Updater, APP_MANAGEMENT, RELEASES};

/// How often a running app asks whether its daily look is due (it is, once a day).
const LOOK_EVERY: Duration = Duration::from_secs(60 * 60);

impl Workspace {
    /// Have this workspace look for updates on its own (the app's, never a test's unless it asks),
    /// keeping its `updates.json` at `store`, for the install `find` gives.
    pub fn enable_updates(&mut self, store: Option<PathBuf>, find: Find) {
        self.updates = Arc::new(Updater::new(store, find, self.wake.clone()));
        self.updates_auto = true;
    }

    /// The daily look, once the settings say whether it is wanted, and at most every
    /// `LOOK_EVERY` (`Updater::daily` decides whether a day has gone by).
    pub(super) fn look_for_updates(&mut self) {
        let Some(enabled) = self.check_for_updates.filter(|_| self.updates_auto) else { return };
        if self.updates_looked.is_some_and(|t| t.elapsed() < LOOK_EVERY) {
            return;
        }
        self.updates_looked = Some(Instant::now());
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
        self.updates.daily(enabled, now);
    }

    /// The notice as `prepare` saw it, when there is one to show (or one fading out, which takes
    /// no clicks).
    pub(super) fn render_update(&self, cx: &mut Context<Self>) -> Option<gpui::AnyElement> {
        let (notice, look, live) = self.notice_drawn()?;
        let notice = notice.clone();
        let t = &self.theme;
        let button = |name: &'static str, label: &'static str, primary: bool| {
            div()
                .id(name)
                .debug_selector(move || sel(name.into()))
                .px(px(10.))
                .py(px(4.))
                .rounded(px(5.))
                .cursor_pointer()
                .bg(rgb(if primary { t.accent } else { t.hover }))
                .text_color(rgb(if primary { t.chrome } else { t.title }))
                .child(label)
        };
        let has_actions = !notice.actions.is_empty();
        let mut row = div().mt(px(10.)).flex().flex_wrap().gap(px(8.));
        for act in notice.actions {
            let (name, label, primary) = match &act {
                Act::Install => ("update-install", "Install", true),
                Act::Notes(_) => ("update-notes", "Release Notes", false),
                Act::Copy(_) => ("update-copy", "Copy", true),
                Act::Restart => ("update-restart", "Restart", true),
                Act::AppManagement => ("update-settings", "Open System Settings", true),
                Act::ReleasePage => ("update-page", "Release Page", false),
                Act::Dismiss => ("update-dismiss", "Dismiss", false),
            };
            row = row.child(button(name, label, primary).when(live, |b| {
                b.on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |ws, _, window, cx| {
                        cx.stop_propagation();
                        ws.update_act(&act, window, cx);
                    }),
                )
            }));
        }
        let card = div()
            .id("update-notice")
            .debug_selector(|| sel("update-notice".into()))
            .w(px(340.))
            .p(px(12.))
            .bg(rgb(t.chrome))
            .border_1()
            .border_color(rgb(t.border))
            .rounded(px(8.))
            .shadow_lg()
            .when(live, |d| d.occlude())
            .text_color(rgb(t.text))
            .child(div().child(notice.text))
            // a card with nothing to click (a step of an install) is the text alone
            .when(has_actions, |card| card.child(row));
        let card = super::presence::presented(look, 340., card);
        Some(div().absolute().bottom(px(16.)).right(px(16.)).child(card).into_any_element())
    }

    /// The palette's rows for what the notice offers now, past Install (always a row): the
    /// keyboard's way to every button on it.
    pub(super) fn update_items(&self) -> Vec<Item> {
        let acts = self.notice().map(|n| n.actions).unwrap_or_default();
        let item = |(name, title): (&str, &str)| Item {
            name: name.into(),
            title: title.into(),
            keywords: vec!["update".into()],
            shortcut: None,
            args: None,
        };
        acts.iter().filter_map(palette_row).map(item).collect()
    }

    /// `client.update.<what>` for one of the notice's buttons: what that button does, if the
    /// notice has it now.
    pub(super) fn update_named(&mut self, name: &str, window: &mut Window, cx: &mut Context<Self>) {
        let acts = self.notice().map(|n| n.actions).unwrap_or_default();
        if let Some(act) = acts.into_iter().find(|a| palette_row(a).is_some_and(|(n, _)| n == name)) {
            self.update_act(&act, window, cx);
        }
    }

    fn update_act(&mut self, act: &Act, window: &mut Window, cx: &mut Context<Self>) {
        match act {
            Act::Install => self.run_action("client.update.install", None, "update_notice", window, cx),
            Act::Restart => self.run_action("client.update.restart", None, "update_notice", window, cx),
            Act::Notes(url) => cx.open_url(url),
            Act::Copy(text) => cx.write_to_clipboard(gpui::ClipboardItem::new_string(text.clone())),
            Act::AppManagement => cx.open_url(APP_MANAGEMENT),
            Act::ReleasePage => cx.open_url(RELEASES),
            Act::Dismiss => self.dismiss_notice(),
        }
        cx.notify();
    }

    /// `client.update.restart`: once an update is installed, run the app again as Start It Again
    /// does. The server is left alone, so the sessions are there when the new app attaches.
    pub(super) fn restart_for_update(&mut self, cx: &mut Context<Self>) {
        if self.updates.notice().is_some_and(|n| n.actions.contains(&Act::Restart)) {
            self.start_again(cx);
        }
    }
}

/// A notice's button as a palette row (action name, title); Install has a row of its own.
fn palette_row(act: &Act) -> Option<(&'static str, &'static str)> {
    Some(match act {
        Act::Install => return None,
        Act::Notes(_) => ("client.update.notes", "Open the Release Notes"),
        Act::Copy(_) => ("client.update.copy", "Copy the Update Command"),
        Act::Restart => ("client.update.restart", "Restart to Finish the Update"),
        Act::AppManagement => ("client.update.settings", "Open App Management Settings"),
        Act::ReleasePage => ("client.update.page", "Open the Release Page"),
        Act::Dismiss => ("client.update.dismiss", "Dismiss the Update Notice"),
    })
}
